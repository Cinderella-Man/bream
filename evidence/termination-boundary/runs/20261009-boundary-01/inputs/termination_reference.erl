%% Bounded correctness controls. Every wait is a protocol gate; the runner owns
%% the watchdog. No sleep, elapsed-time observation, benchmark or native NIF.
-module(termination_reference).
-export([run/0]).

run() ->
    try
        Cases = [owned_kill, ets_cancel, ets_complete, ets_atomic_cancel,
                 unlinked_child, linked_child],
        Results = [run_case(Case) || Case <- Cases],
        io:format("~s~n", [json:encode(#{cases => Results,
                                       otp_release => erlang:system_info(otp_release),
                                       emu_flavor => erlang:system_info(emu_flavor)})]),
        halt(0)
    catch
        Class:Reason:Stack ->
            io:format(standard_error, "~p:~p ~p~n", [Class, Reason, Stack]),
            halt(1)
    end.

run_case(owned_kill) ->
    Host = self(),
    Initial = {100, 100},
    {Peer, PeerRef} = spawn_monitor(fun() ->
        receive read -> Host ! {peer_total, total(Initial)} end
    end),
    {Actor, Ref} = spawn_monitor(fun() ->
        {Left, Right} = Initial,
        Local = {Left - 1, Right},
        Host ! {ready, self(), Local},
        receive finish -> ok end
    end),
    receive {ready, Actor, {99, 100}} -> ok end,
    exit(Actor, kill),
    down(Ref, Actor, killed),
    Peer ! read,
    receive {peer_total, 200} -> ok end,
    down(PeerRef, Peer, normal),
    #{'case' => owned_kill, peer_total => 200};
run_case(Case) when Case =:= ets_cancel; Case =:= ets_complete; Case =:= ets_atomic_cancel ->
    Host = self(),
    Table = ets:new(ledger, [set, public]),
    true = ets:insert(Table, {balances, 100, 100}),
    {Actor, Ref} = spawn_monitor(fun() ->
        Right = case Case of ets_atomic_cancel -> 101; _ -> 100 end,
        true = ets:insert(Table, {balances, 99, Right}),
        Host ! {ready, self()},
        receive finish -> ets:insert(Table, {balances, 99, 101}) end
    end),
    receive {ready, Actor} -> ok end,
    case Case of
        ets_complete -> Actor ! finish, down(Ref, Actor, normal);
        _ -> exit(Actor, kill), down(Ref, Actor, killed)
    end,
    {Peer, PeerRef} = spawn_monitor(fun() ->
        [{balances, Left, Right}] = ets:lookup(Table, balances),
        Host ! {peer_total, Left + Right}
    end),
    Expected = case Case of ets_cancel -> 199; _ -> 200 end,
    receive {peer_total, Expected} -> ok end,
    down(PeerRef, Peer, normal),
    true = ets:delete(Table),
    #{'case' => Case, peer_total => Expected};
run_case(Case) when Case =:= unlinked_child; Case =:= linked_child ->
    Host = self(),
    {Actor, Ref} = spawn_monitor(fun() ->
        ChildFun = fun() -> child(Host) end,
        Child = case Case of
            unlinked_child -> spawn(ChildFun);
            linked_child -> spawn_link(ChildFun)
        end,
        Host ! {ready, self(), Child},
        receive finish -> ok end
    end),
    Child = receive {ready, Actor, Pid} -> Pid end,
    ChildRef = monitor(process, Child),
    exit(Actor, kill),
    down(Ref, Actor, killed),
    Effects = case Case of
        unlinked_child ->
            Child ! work,
            receive {child_effect, Child, 1} -> ok end,
            Child ! stop,
            down(ChildRef, Child, normal),
            1;
        linked_child ->
            down(ChildRef, Child, killed),
            false = is_process_alive(Child),
            0
    end,
    #{'case' => Case, child_effects_after_parent_down => Effects}.

child(Host) ->
    receive
        work -> Host ! {child_effect, self(), 1}, child(Host);
        stop -> ok
    end.

total({Left, Right}) -> Left + Right.

down(Ref, Pid, Reason) ->
    receive {'DOWN', Ref, process, Pid, Reason} -> ok end.
