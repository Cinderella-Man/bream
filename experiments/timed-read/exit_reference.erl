%% Correctness-only OTP oracle. Deadlines detect hangs; no durations are read.
%% JSON schema: {otp_release:string, emu_flavor:string, cases:[row]}.
%% Reason codes: normal=1, killed=2, failure=3, noproc=4, kill=5.
%% Matrix mode: self_exit=1, legacy self-target exit/2=2, peer exit/2=3.
%% Outcome: ignored=0, EXIT message=1, terminal=2. observed_reason=0 for ignored.
%% returned refers to the operation, not receipt of a termination notification.
%% There are 39 rows: 30 matrix, 4 chain, 2 catch, 1 dead target, 2 ordering.
-module(exit_reference).
-export([run/0]).

run() ->
    try
        Matrix = [matrix(Mode, Reason, Trap)
                  || Mode <- [1, 2, 3], Reason <- [1, 2, 3, 4, 5], Trap <- [0, 1]],
        Chains = [chain(Direct, Trap) || Direct <- [0, 1], Trap <- [0, 1]],
        Cases = Matrix ++ Chains ++ catches() ++ [dead_target()] ++
                [ordering(Trap) || Trap <- [0, 1]],
        39 = length(Cases),
        {messages, []} = process_info(self(), messages),
        io:format("~s~n", [json:encode(#{otp_release => list_to_binary(erlang:system_info(otp_release)),
                                       emu_flavor => erlang:system_info(emu_flavor),
                                       cases => Cases})]),
        halt(0)
    catch
        Class:Reason:Stack ->
            io:format(standard_error, "~p:~p ~p~n", [Class, Reason, Stack]),
            halt(1)
    end.

reason(1) -> normal;
reason(2) -> killed;
reason(3) -> failure;
reason(4) -> noproc;
reason(5) -> kill.

matrix(Mode, Code, Trap) ->
    Host = self(),
    Reason = reason(Code),
    {Actor, Ref} = spawn_monitor(fun() ->
        process_flag(trap_exit, Trap =:= 1),
        Host ! {ready, self()},
        receive go -> ok after 10000 -> error(go_watchdog) end,
        matrix_actor(Mode, Reason, Host)
    end),
    ready(Actor),
    Actor ! go,
    case Mode of
        3 -> true = exit(Actor, Reason), Actor ! probe;
        _ -> ok
    end,
    {Outcome, Observed, Returned} = expected(Mode, Code, Trap),
    case {Outcome, Mode} of
        {2, _} ->
            down(Ref, Actor, reason(Observed));
        {1, 2} ->
            receive {returned, Actor} -> ok after 10000 -> error(return_watchdog) end,
            receive {observed, Actor, Actor, Reason} -> ok
            after 10000 -> error(self_exit_message_watchdog) end,
            Actor ! stop,
            down(Ref, Actor, normal);
        {1, 3} ->
            receive {observed, Actor, Host, Reason} -> ok
            after 10000 -> error(peer_exit_message_watchdog) end,
            probe(Actor),
            Actor ! stop,
            down(Ref, Actor, normal);
        {0, 3} ->
            probe(Actor),
            Actor ! stop,
            down(Ref, Actor, normal)
    end,
    #{kind => matrix, mode => Mode, reason => Code, trap => Trap,
      outcome => Outcome, observed_reason => Observed, returned => Returned}.

expected(1, Code, _Trap) -> {2, Code, 0};
expected(2, 5, _Trap) -> {2, 2, 0};
expected(2, Code, 0) -> {2, Code, 0};
expected(2, Code, 1) -> {1, Code, 1};
expected(3, 5, _Trap) -> {2, 2, 1};
expected(3, 1, 0) -> {0, 0, 1};
expected(3, Code, 0) -> {2, Code, 1};
expected(3, Code, 1) -> {1, Code, 1}.

matrix_actor(1, Reason, _Host) ->
    exit(Reason);
matrix_actor(2, Reason, Host) ->
    true = exit(self(), Reason),
    Host ! {returned, self()},
    Self = self(),
    receive {'EXIT', Self, Reason} -> Host ! {observed, Self, Self, Reason}
    after 10000 -> error(self_signal_watchdog) end,
    receive stop -> ok after 10000 -> error(stop_watchdog) end;
matrix_actor(3, _Reason, Host) ->
    peer_loop(Host).

peer_loop(Host) ->
    receive
        {'EXIT', Sender, Reason} ->
            Host ! {observed, self(), Sender, Reason},
            peer_loop(Host);
        probe -> Host ! {probe, self()}, peer_loop(Host);
        stop -> ok
    after 10000 -> error(peer_watchdog)
    end.

chain(Direct, Trap) ->
    Host = self(),
    {Sink, SinkRef} = spawn_monitor(fun() ->
        process_flag(trap_exit, Trap =:= 1),
        Host ! {ready, self()},
        receive
            {'EXIT', From, Why} ->
                Host ! {chain_exit, self(), From, Why},
                receive stop -> ok after 10000 -> error(chain_stop_watchdog) end
        after 10000 -> error(chain_sink_watchdog)
        end
    end),
    ready(Sink),
    {Middle, MiddleRef} = spawn_monitor(fun() ->
        true = link(Sink),
        Host ! {ready, self()},
        receive stop -> ok after 10000 -> error(chain_middle_watchdog) end
    end),
    ready(Middle),
    {Source, SourceRef} = spawn_monitor(fun() ->
        true = link(Middle),
        Host ! {ready, self()},
        receive self_exit -> exit(kill)
        after 10000 -> error(chain_source_watchdog) end
    end),
    ready(Source),
    Code = case Direct of
        0 -> Source ! self_exit, 5;
        1 -> true = exit(Source, kill), 2
    end,
    Why = reason(Code),
    down(SourceRef, Source, Why),
    down(MiddleRef, Middle, Why),
    Outcome = case Trap of
        0 -> down(SinkRef, Sink, Why), 2;
        1 ->
            receive {chain_exit, Sink, Middle, Why} -> ok
            after 10000 -> error(chain_message_watchdog) end,
            Sink ! stop,
            down(SinkRef, Sink, normal),
            1
    end,
    #{kind => chain, direct_kill => Direct, trap => Trap,
      source_reason => Code, middle_reason => Code,
      outcome => Outcome, observed_reason => Code}.

catches() ->
    kill = try exit(kill) catch exit:Caught -> Caught end,
    Host = self(),
    {Actor, Ref} = spawn_monitor(fun() ->
        Result = try exit(self(), kill) catch exit:CaughtSelf -> CaughtSelf end,
        Host ! {unexpected_catch, self(), Result}
    end),
    down(Ref, Actor, killed),
    [#{kind => 'catch', mode => 1, caught => 1, observed_reason => 5},
     #{kind => 'catch', mode => 2, caught => 0, observed_reason => 2}].

dead_target() ->
    {Actor, Ref} = spawn_monitor(fun() -> ok end),
    down(Ref, Actor, normal),
    false = is_process_alive(Actor),
    [true, true, true, true, true] = [exit(Actor, reason(Code)) || Code <- [1, 2, 3, 4, 5]],
    #{kind => dead_target, returned_count => 5, target_alive => 0}.

ordering(Trap) ->
    Host = self(),
    {Actor, Ref} = spawn_monitor(fun() ->
        process_flag(trap_exit, Trap =:= 1),
        Host ! {ready, self()},
        case Trap of
            0 -> fatal_order_loop();
            1 ->
                [before_token, {'EXIT', Host, failure}, after_token] =
                    [next_message(), next_message(), next_message()],
                Host ! {ordered, self()},
                receive stop -> ok after 10000 -> error(order_stop_watchdog) end
        end
    end),
    ready(Actor),
    Actor ! before_token,
    true = exit(Actor, failure),
    Actor ! after_token,
    case Trap of
        0 -> down(Ref, Actor, failure);
        1 ->
            receive {ordered, Actor} -> ok after 10000 -> error(order_watchdog) end,
            Actor ! stop,
            down(Ref, Actor, normal)
    end,
    #{kind => ordering, trap => Trap, same_sender_order => 1}.

fatal_order_loop() ->
    receive
        before_token -> fatal_order_loop();
        after_token -> error(later_message_crossed_fatal_exit)
    after 10000 -> error(fatal_order_watchdog)
    end.

next_message() ->
    receive Message -> Message after 10000 -> error(message_watchdog) end.

ready(Actor) ->
    receive {ready, Actor} -> ok after 10000 -> error(ready_watchdog) end.

probe(Actor) ->
    receive {probe, Actor} -> ok after 10000 -> error(probe_watchdog) end.

down(Ref, Actor, Expected) ->
    receive
        {'DOWN', Ref, process, Actor, Expected} -> ok;
        {'DOWN', Ref, process, Actor, Other} -> error({wrong_reason, Expected, Other})
    after 10000 -> error(down_watchdog)
    end.
