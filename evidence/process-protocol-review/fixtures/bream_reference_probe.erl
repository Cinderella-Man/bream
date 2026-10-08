%% Bounded process-semantics observations for the pinned OTP build.
%% No OTP behaviour framework and no Bream implementation is involved.
-module(bream_reference_probe).
-export([run/1]).

run([Output]) ->
    Cases = [
        {synthetic_flush_active, fun() -> synthetic_flush([flush]) end},
        {synthetic_flush_info, fun() -> synthetic_flush([flush, info]) end},
        {synthetic_flush_expansion, fun synthetic_expansion/0},
        {real_down_flush, fun real_down_flush/0},
        {dead_link, fun dead_link/0},
        {dead_suspend, fun dead_suspend/0},
        {notification_order, fun notification_order/0},
        {paused_timer_identity, fun paused_timer_identity/0},
        {timer_creator_death, fun timer_creator_death/0},
        {receive_timeout_validation, fun receive_timeout_validation/0},
        {fullsweep_roundtrip, fun fullsweep_roundtrip/0}
    ],
    Results = [run_case(Name, F) || {Name, F} <- Cases],
    Meta = #{otp_release => unicode:characters_to_binary(erlang:system_info(otp_release)),
             erts_version => unicode:characters_to_binary(erlang:system_info(version)),
             system_version => unicode:characters_to_binary(erlang:system_info(system_version)),
             emu_flavor => erlang:system_info(emu_flavor),
             build_type => erlang:system_info(build_type),
             schedulers => erlang:system_info(schedulers),
             schedulers_online => erlang:system_info(schedulers_online),
             root_dir => unicode:characters_to_binary(code:root_dir())},
    ok = file:write_file(Output, json:encode(normalize(#{metadata => Meta, results => Results}))),
    case lists:all(fun(#{status := S}) -> S =:= completed end, Results) of
        true -> halt(0);
        false -> halt(2)
    end;
run([Output, "hibernate", CountString]) ->
    Count=list_to_integer(CountString),
    R=run_case({hibernate_arity,Count},fun()->hibernate_arity(Count) end),
    ok=file:write_file(Output,json:encode(normalize(R))),
    halt().

run_case(Name,F) ->
    Parent=self(),
    {P,M}=spawn_monitor(fun()->
        put(children,[]),
        R=try F() of V -> #{status=>completed,observations=>V}
          catch C:E:S -> #{status=>fixture_error,class=>C,reason=>E,stack=>S}
          after cleanup() end,
        Parent!{case_result,self(),R}
    end),
    receive
        {case_result,P,R} ->
            receive {'DOWN',M,process,P,normal}->ok after 1000->error(case_exit_missing) end,
            R#{name=>Name};
        {'DOWN',M,process,P,R} -> #{name=>Name,status=>fixture_crashed,reason=>R}
    after 15000 -> exit(P,kill), #{name=>Name,status=>fixture_timeout}
    end.

new(F) -> P=spawn(F), put(children,[P|get(children)]), P.
new_monitor(F) -> {P,M}=spawn_monitor(F), put(children,[P|get(children)]), {P,M}.
cleanup() -> lists:foreach(fun(P)->unlink(P),exit(P,kill) end,get(children)).
hold() -> receive stop->ok; _->hold() end.
messages() -> {messages,L}=process_info(self(),messages), L.
catch_result(F) -> try F() of V->{returned,V} catch C:R->{raised,C,R} end.
wait_until(F) -> wait_until(F,erlang:monotonic_time(millisecond)+3000).
wait_until(F,D) ->
    case F() of
        true->ok;
        false->case erlang:monotonic_time(millisecond)<D of
            true->receive after 1->ok end,wait_until(F,D);
            false->error(wait_timeout)
        end
    end.

dead_pid() ->
    P=new(fun()->ok end), M=monitor(process,P),
    receive {'DOWN',M,process,P,_}->P after 3000->error(no_death) end.

synthetic_flush(Options) ->
    P=new(fun hold/0), R=monitor(process,P), Fake={synthetic,R,a,b,c},
    self()!Fake,
    First=demonitor(R,Options), FirstMsgs=messages(),
    Second=demonitor(R,[flush,info]), SecondMsgs=messages(),
    #{first_return=>First,synthetic_after_first=>lists:member(Fake,FirstMsgs),
      second_return=>Second,synthetic_after_second=>lists:member(Fake,SecondMsgs),
      target_alive=>is_process_alive(P)}.

synthetic_expansion() ->
    P=new(fun hold/0), R=monitor(process,P), Fake={synthetic,R,a,b,c},self()!Fake,
    D=demonitor(R),
    Removed=receive {_,R,_,_,_}->true after 0->false end,
    #{demonitor_return=>D,removed=>Removed,remaining=>messages(),target_alive=>is_process_alive(P)}.

real_down_flush() ->
    P=new(fun()->receive go->exit(probe_reason) end end),R=monitor(process,P),P!go,
    wait_until(fun()->lists:any(fun(X)->case X of {'DOWN',R,process,P,_}->true;_->false end end,messages()) end),
    Before=messages(), D=demonitor(R,[flush,info]),
    #{down_before=>length(Before),return=>D,remaining=>messages()}.

dead_link() ->
    P=dead_pid(),Untrapped=catch_result(fun()->link(P) end),
    process_flag(trap_exit,true),Trapped=catch_result(fun()->link(P) end),
    Msg=receive {'EXIT',P,R}->R after 3000->missing end,
    #{untrapped=>Untrapped,trapped=>Trapped,exit_reason=>Msg}.

dead_suspend() ->
    P=dead_pid(),Tag=make_ref(),
    Tagged=catch_result(fun()->erlang:suspend_process(P,[{asynchronous,Tag}]) end),
    Reply=receive {Tag,R}->R after 3000->missing end,
    Untagged=catch_result(fun()->erlang:suspend_process(P,[asynchronous]) end),
    Sync=catch_result(fun()->erlang:suspend_process(P) end),
    #{tagged_return=>Tagged,tagged_reply=>Reply,untagged_return=>Untagged,synchronous=>Sync}.

notification_order() ->
    process_flag(trap_exit,true),
    [#{link_priority=>L,monitor_priority=>M,sequence_counts=>counts([notification_once(L,M)||_<-lists:seq(1,100)])}
     || {L,M}<-[{false,false},{false,true},{true,false},{true,true}]].
notification_once(L,M) ->
    P=new(fun()->receive go->exit(order_reason) end end),
    true=link(P,case L of true->[priority];false->[] end),
    R=monitor(process,P,case M of true->[priority];false->[] end),
    Marker=make_ref(),self()!{before,Marker},P!go,
    wait_until(fun()->
        Ms=messages(),
        lists:member({'EXIT',P,order_reason},Ms) andalso lists:member({'DOWN',R,process,P,order_reason},Ms)
    end),
    [receive {before,Marker}->before;{'EXIT',P,order_reason}->exit;{'DOWN',R,process,P,order_reason}->down after 0->missing end ||_<-lists:seq(1,3)].
counts(Values) ->
    M=lists:foldl(fun(V,A)->maps:update_with(V,fun(N)->N+1 end,1,A) end,#{},Values),
    [#{sequence=>K,count=>V}||{K,V}<-lists:sort(maps:to_list(M))].

paused_timer_identity() ->
    Parent=self(),P=new(fun()->forward(Parent) end), Duration=1000,
    Start=erlang:monotonic_time(millisecond),
    TRead=erlang:start_timer(Duration,P,read_only),
    TSend=erlang:send_after(Duration,P,send_read_only),
    TCancelPaused=erlang:start_timer(Duration,P,cancel_paused),
    TCancelResumed=erlang:start_timer(Duration,P,cancel_resumed),
    Before=[erlang:read_timer(T)||T<-[TRead,TSend,TCancelPaused,TCancelResumed]],
    true=lists:all(fun(V)->is_integer(V) andalso V>500 end,Before),
    true=erlang:suspend_process(P),
    SuspendElapsed=erlang:monotonic_time(millisecond)-Start,
    true=(SuspendElapsed<500),
    PausedRead=[erlang:read_timer(T)||T<-[TRead,TSend,TCancelResumed]],
    PausedCancel=erlang:cancel_timer(TCancelPaused),
    TDuringRead=erlang:start_timer(Duration,P,during_read_only),
    TDuringCancel=erlang:start_timer(Duration,P,during_cancel),
    DuringRead=erlang:read_timer(TDuringRead),
    DuringCancel=erlang:cancel_timer(TDuringCancel),
    ResumeStart=erlang:monotonic_time(millisecond),
    true=erlang:resume_process(P),
    ResumedRead=[erlang:read_timer(T)||T<-[TRead,TSend,TDuringRead]],
    ResumedCancel=erlang:cancel_timer(TCancelResumed),
    ResumeReadElapsed=erlang:monotonic_time(millisecond)-ResumeStart,
    true=(ResumeReadElapsed<500),
    Delivered=collect_forwarded(P,erlang:monotonic_time(millisecond)+2000,[]),
    #{pre_suspend_remaining_ms=>Before,suspend_elapsed_ms=>SuspendElapsed,
      resume_read_elapsed_ms=>ResumeReadElapsed,
      paused_read_only_refs=>PausedRead,paused_cancel_separate_ref=>PausedCancel,
      created_during_pause_read=>DuringRead,created_during_pause_cancel_separate_ref=>DuringCancel,
      resumed_read_only_refs=>ResumedRead,resumed_cancel_separate_ref=>ResumedCancel,
      untouched_start_delivered_original_ref=>lists:member({timeout,TRead,read_only},Delivered),
      untouched_send_after_delivered=>lists:member(send_read_only,Delivered),
      untouched_during_pause_delivered_original_ref=>lists:member({timeout,TDuringRead,during_read_only},Delivered),
      canceled_while_paused_delivered=>lists:member({timeout,TCancelPaused,cancel_paused},Delivered),
      canceled_after_resume_delivered=>lists:member({timeout,TCancelResumed,cancel_resumed},Delivered),
      created_and_canceled_while_paused_delivered=>lists:member({timeout,TDuringCancel,during_cancel},Delivered),
      delivery_count=>length(Delivered)}.
collect_forwarded(P,Deadline,Acc) ->
    Left=max(0,Deadline-erlang:monotonic_time(millisecond)),
    receive {forwarded,P,X}->collect_forwarded(P,Deadline,[X|Acc])
    after Left->lists:reverse(Acc)
    end.

forward(Parent) -> receive X->Parent!{forwarded,self(),X},forward(Parent) end.

timer_creator_death() ->
    Parent=self(),Recipient=new(fun()->forward(Parent) end),
    Creator=new(fun()->Ref=erlang:start_timer(150,Recipient,persist),Parent!{created,self(),Ref} end),
    Mon=monitor(process,Creator),
    Ref=receive {created,Creator,R}->R after 3000->error(no_timer) end,
    receive {'DOWN',Mon,process,Creator,_}->ok after 3000->error(no_creator_death) end,
    Delivered=receive {forwarded,Recipient,{timeout,Ref,persist}}->true after 3000->false end,
    #{creator_dead=>not is_process_alive(Creator),delivered=>Delivered}.

receive_timeout_validation() ->
    self()!match,
    Queued=invalid_timeout(invalid),
    Ref=make_ref(),self()!{old,Ref},
    Empty=catch_result(fun()->invalid_timeout(invalid) end),
    Recovery=receive {old,Ref}->recovered after 0->missing end,
    #{queued_match=>Queued,empty_queue=>Empty,next_receive=>Recovery}.
invalid_timeout(T) -> receive match->selected after T->timed_out end.

fullsweep_roundtrip() ->
    [begin Old=process_flag(fullsweep_after,N),
       {garbage_collection,GC}=process_info(self(),garbage_collection),
       Observed=proplists:get_value(fullsweep_after,GC),
       process_flag(fullsweep_after,Old),#{requested=>N,observed=>Observed}
     end || N<-[0,1,65535,65536,65537]].

hibernate_arity(N) ->
    Parent=self(), {P,M}=new_monitor(fun()->
        Parent!{entering,self()},
        try erlang:hibernate(erlang,length,lists:duplicate(N,arg)) of V->Parent!{caught,self(),{returned,V}}
        catch C:R->Parent!{caught,self(),{raised,C,R}}
        end
    end),
    receive {entering,P}->ok after 3000->error(no_entry) end,
    P!wake,
    receive
        {caught,P,R}->#{arity=>N,phase=>caught_before_stack_discard,result=>R};
        {'DOWN',M,process,P,R}->#{arity=>N,phase=>uncaught_after_hibernate,reason=>summarize_reason(R)}
    after 3000->#{arity=>N,phase=>still_alive,info=>process_info(P,[current_function,status])}
    end.
summarize_reason({Reason,[{Mod,Fun,Args,_}|_]}) when is_list(Args) ->
    #{reason=>Reason,top_module=>Mod,top_function=>Fun,top_arity=>length(Args)};
summarize_reason(R)->R.

normalize(M) when is_map(M) -> maps:from_list([{key(K),normalize(V)}||{K,V}<-maps:to_list(M)]);
normalize(T) when is_tuple(T) -> [normalize(X)||X<-tuple_to_list(T)];
normalize(L) when is_list(L) -> [normalize(X)||X<-L];
normalize(A) when A=:=true; A=:=false; A=:=null -> A;
normalize(A) when is_atom(A) -> atom_to_binary(A);
normalize(B) when is_binary(B); is_number(B) -> B;
normalize(Other) -> iolist_to_binary(io_lib:format("~0p",[Other])).
key(A) when is_atom(A)->atom_to_binary(A);
key(B) when is_binary(B)->B;
key(X)->iolist_to_binary(io_lib:format("~0p",[X])).
