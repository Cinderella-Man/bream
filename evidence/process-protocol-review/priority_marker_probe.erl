%% Source-derived qualification fixture; not an asserted OTP bug.
%% Compile this module normally and again with no_recv_opt into separate dirs.
%% Inspect probe/0: reserve/bind must precede make_ref/0, and marker use for R
%% must remain at the receive after unalias/1. If that use is absent, a passing
%% run does not exercise the suspected debug assertion.
%% Run in a disposable VM: a DEBUG assertion can terminate the whole VM.
-module(priority_marker_probe).
-export([run/0, probe/0]).

run() ->
    Parent = self(),
    {Pid, MRef} = spawn_monitor(fun () ->
                                       Parent ! {self(), result, probe()}
                               end),
    receive
        {Pid, result, Result} ->
            receive
                {'DOWN', MRef, process, Pid, normal} -> {ok, Result};
                {'DOWN', MRef, process, Pid, Reason} -> {error, Reason}
            after 5000 ->
                exit(Pid, kill),
                {error, worker_exit_timeout}
            end;
        {'DOWN', MRef, process, Pid, Reason} ->
            {error, Reason}
    after 5000 ->
        exit(Pid, kill),
        {error, worker_result_timeout}
    end.

probe() ->
    %% Keep an old ordinary message before R's reserve boundary. It prevents
    %% leading-marker reclamation after the barrier receive from trivially
    %% erasing the marker we need to exercise.
    self() ! sentinel,
    R = make_ref(),
    A = alias([explicit_unalias, priority]),
    Receiver = self(),
    Sender = spawn(fun () ->
                           erlang:send(A, {R, reply}, [priority]),
                           Receiver ! {self(), accepted_barrier}
                   end),
    %% Same-sender signal order establishes that the alias message has been
    %% handled before this ordinary barrier is selected. No sleeps are used.
    receive
        {Sender, accepted_barrier} -> ok
    end,
    {priority_messages, true} = process_info(self(), priority_messages),
    true = unalias(A),
    {priority_messages, false} = process_info(self(), priority_messages),
    Reply = receive
                {R, Value} -> Value
            after 0 ->
                error(priority_reply_was_skipped)
            end,
    Remaining = drain(),
    reply = Reply,
    [sentinel] = Remaining,
    #{reply => Reply, remaining => Remaining, priority_messages => false}.

drain() ->
    receive
        Message -> [Message | drain()]
    after 0 ->
        []
    end.
