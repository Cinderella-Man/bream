-module(name_routing_probe).
-export([run/0]).

run() ->
    io:format("VM otp=~s erts=~s schedulers=~p~n",
              [erlang:system_info(otp_release), erlang:system_info(version),
               erlang:system_info(schedulers_online)]),
    Cases = [{cached_vs_dynamic, fun cached_vs_dynamic/0},
             {named_monitor_incarnation, fun named_monitor_incarnation/0},
             {missing_named_monitor, fun missing_named_monitor/0},
             {missing_address_forms, fun missing_address_forms/0},
             {detached_name_snapshot, fun detached_name_snapshot/0},
             {timer_late_binding, fun timer_late_binding/0},
             {competing_names, fun() -> repeat(50, fun competing_names/0) end},
             {competing_targets, fun() -> repeat(50, fun competing_targets/0) end}],
    lists:foreach(fun({Name, Test}) ->
        ok = Test(), io:format("PASS ~s~n", [Name])
    end, Cases),
    receive Unexpected -> erlang:error({unexpected_message, Unexpected})
    after 0 -> ok end,
    io:format("COMPLETE families=8 race_rounds=100~n"),
    halt(0).

repeat(0, _) -> ok;
repeat(N, F) -> ok = F(), repeat(N - 1, F).

worker() ->
    receive
        {deliver, Caller, Ref} -> Caller ! {delivered, Ref, self()}, worker()
    end.

start() -> spawn(fun worker/0).

kill(Pid) ->
    Ref = monitor(process, Pid),
    exit(Pid, kill),
    receive {'DOWN', Ref, process, Pid, _} -> ok
    after 3000 -> erlang:error(kill_timeout) end.

delivered(Ref, Pid) ->
    receive {delivered, Ref, Pid} -> ok
    after 3000 -> erlang:error({delivery_timeout, Pid}) end.

cached_vs_dynamic() ->
    A = start(), B = start(),
    true = register(bream_route, A),
    A = whereis(bream_route),
    Cached = whereis(bream_route),
    true = unregister(bream_route),
    true = register(bream_route, B),
    R1 = make_ref(), R2 = make_ref(), R3 = make_ref(),
    Cached ! {deliver, self(), R1},
    bream_route ! {deliver, self(), R2},
    {bream_route, node()} ! {deliver, self(), R3},
    ok = delivered(R1, A), ok = delivered(R2, B), ok = delivered(R3, B),
    kill(A), B = whereis(bream_route), kill(B).

named_monitor_incarnation() ->
    A = start(), B = start(), true = register(bream_route, A),
    Ref = monitor(process, bream_route),
    true = unregister(bream_route), true = register(bream_route, B),
    kill(A),
    receive {'DOWN', Ref, process, {bream_route, Node}, killed} when Node =:= node() -> ok
    after 3000 -> erlang:error(named_down_timeout) end,
    B = whereis(bream_route), true = is_process_alive(B), kill(B).

missing_named_monitor() ->
    undefined = whereis(bream_route),
    Ref = monitor(process, bream_route),
    B = start(), true = register(bream_route, B),
    receive {'DOWN', Ref, process, {bream_route, Node}, noproc} when Node =:= node() -> ok
    after 3000 -> erlang:error(missing_down_timeout) end,
    B = whereis(bream_route), kill(B).

missing_address_forms() ->
    undefined = whereis(bream_route),
    badarg = try bream_route ! absent, unexpected catch error:badarg -> badarg end,
    absent = ({bream_route, node()} ! absent),
    ok = erlang:send({bream_route, node()}, absent, [noconnect]),
    A = start(), kill(A), absent = (A ! absent), ok.

detached_name_snapshot() ->
    A = start(), true = register(bream_route, A),
    Names = registered(), true = lists:member(bream_route, Names),
    true = unregister(bream_route),
    undefined = whereis(bream_route),
    true = lists:member(bream_route, Names), kill(A).

timer_late_binding() ->
    A = start(), B = start(), true = register(bream_route, A),
    Ref = make_ref(),
    Timer = erlang:send_after(250, bream_route, {deliver, self(), Ref}),
    true = unregister(bream_route), true = register(bream_route, B),
    Remaining = erlang:read_timer(Timer),
    %% An expired timer invalidates setup; it must never count as a pass.
    true = is_integer(Remaining) andalso Remaining > 0,
    ok = delivered(Ref, B), kill(A), kill(B).

compete(Name, Target, Caller, Ref) ->
    receive go -> ok end,
    Result = try register(Name, Target) of true -> won
             catch error:badarg -> lost end,
    Caller ! {registration, Ref, Name, Target, Result}.

result(Ref) ->
    receive {registration, Ref, Name, Target, Result} -> {Name, Target, Result}
    after 3000 -> erlang:error(register_timeout) end.

competing_names() ->
    A = start(), Parent = self(), R1 = make_ref(), R2 = make_ref(),
    C1 = spawn(fun() -> compete(bream_route, A, Parent, R1) end),
    C2 = spawn(fun() -> compete(bream_route_other, A, Parent, R2) end),
    C1 ! go, C2 ! go,
    Results = [result(R1), result(R2)],
    [{Name, A, won}] = [X || X = {_, _, won} <- Results],
    1 = length([X || X = {_, _, lost} <- Results]),
    A = whereis(Name),
    [Name] = [N || N <- [bream_route, bream_route_other], whereis(N) =:= A],
    kill(A), undefined = whereis(bream_route), undefined = whereis(bream_route_other), ok.

competing_targets() ->
    A = start(), B = start(), Parent = self(), R1 = make_ref(), R2 = make_ref(),
    C1 = spawn(fun() -> compete(bream_route, A, Parent, R1) end),
    C2 = spawn(fun() -> compete(bream_route, B, Parent, R2) end),
    C1 ! go, C2 ! go,
    Results = [result(R1), result(R2)],
    [{bream_route, Winner, won}] = [X || X = {_, _, won} <- Results],
    1 = length([X || X = {_, _, lost} <- Results]),
    Winner = whereis(bream_route), kill(A), kill(B),
    undefined = whereis(bream_route), ok.
