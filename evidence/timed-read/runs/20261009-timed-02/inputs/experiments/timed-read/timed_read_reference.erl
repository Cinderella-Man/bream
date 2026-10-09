%% Correctness-only OTP socket oracle. No clock reads or duration output.
%% The finite recv timeout tests an API outcome; all other finite waits are
%% watchdogs. This fixture is not a throughput or latency experiment.
%% Six cases: zero empty, zero data, finite timeout/reuse, infinity data,
%% stale cancellation, and the deliberate read-exactly/read-some difference.
-module(timed_read_reference).
-export([run/0]).

run() ->
    try
        Cases = [with_pair(Case) || Case <-
                    [fun zero_empty/2, fun zero_buffered/2,
                     fun finite_timeout_reuse/2, fun infinite_buffered/2,
                     fun stale_cancel/2, fun read_exactly_difference/2]],
        6 = length(Cases),
        {messages, []} = process_info(self(), messages),
        io:format("~s~n", [json:encode(#{
            otp_release => list_to_binary(erlang:system_info(otp_release)),
            emu_flavor => erlang:system_info(emu_flavor),
            cases => Cases})]),
        halt(0)
    catch
        Class:Reason:Stack ->
            io:format(standard_error, "~p:~p ~p~n", [Class, Reason, Stack]),
            halt(1)
    end.

zero_empty(Socket, _Peer) ->
    {error, timeout} = socket:recv(Socket, 1, [], 0),
    #{kind => zero_empty, timed_out => 1}.

zero_buffered(Socket, Peer) ->
    Data = <<2, 3, 5>>,
    socket_send(Peer, Data),
    {ok, Data} = socket:recv(Socket, byte_size(Data), [], 0),
    #{kind => zero_buffered, data => binary_to_list(Data)}.

finite_timeout_reuse(Socket, Peer) ->
    %% A positive timeout exercises the timed receive branch. Only its result
    %% is asserted; elapsed time is neither read nor compared.
    {error, timeout} = socket:recv(Socket, 1, [], 1),
    Data = <<7, 11, 13>>,
    socket_send(Peer, Data),
    {ok, Data} = socket:recv(Socket, byte_size(Data), [], 0),
    #{kind => finite_timeout_reuse, timed_out => 1,
      endpoint_reused => 1, data => binary_to_list(Data)}.

infinite_buffered(Socket, Peer) ->
    Data = <<17, 19, 23>>,
    socket_send(Peer, Data),
    {ok, Data} = socket:recv(Socket, byte_size(Data), [], infinity),
    #{kind => infinite_buffered, data => binary_to_list(Data)}.

stale_cancel(Socket, Peer) ->
    First = make_ref(),
    {select, FirstInfo = {select_info, recv, First}} =
        socket:recv(Socket, 3, [], First),
    ok = socket:cancel(Socket, FirstInfo),
    Second = make_ref(),
    {select, SecondInfo = {select_info, recv, Second}} =
        socket:recv(Socket, 3, [], Second),
    {error, {invalid, FirstInfo}} = socket:cancel(Socket, FirstInfo),
    Data = <<29, 31, 37>>,
    socket_send(Peer, Data),
    receive
        {'$socket', Socket, select, Second} -> ok;
        {'$socket', Socket, select, First} -> error(stale_select);
        {'$socket', Socket, abort, {Second, Why}} -> error({aborted, Why})
    after 10000 -> error(select_watchdog)
    end,
    {ok, Data} = socket:recv(Socket, byte_size(Data), [], 0),
    {error, {invalid, SecondInfo}} = socket:cancel(Socket, SecondInfo),
    #{kind => stale_cancel, first_cancelled => 1,
      stale_rejected => 1, second_select => 1,
      completed_handle_rejected => 1, data => binary_to_list(Data)}.

read_exactly_difference(Socket, Peer) ->
    Data = <<41, 43>>,
    socket_send(Peer, Data),
    {error, {timeout, Data}} = socket:recv(Socket, 4, [], 0),
    %% The partial bytes were consumed, although the exact-length operation
    %% timed out. Bream's bounded read-some contract instead returns Data.
    {error, timeout} = socket:recv(Socket, 1, [], 0),
    #{kind => read_exactly_difference,
      difference => <<"OTP read-exactly partial timeout; Bream read-some success">>,
      requested => 4, timed_out => 1, data => binary_to_list(Data),
      partial_consumed => 1}.

socket_send(Peer, Data) ->
    ok = socket:send(Peer, Data, [], 10000).

with_pair(Case) ->
    %% Abstract local addresses are Linux-specific and require no file cleanup.
    %% The OS pid separates fixture processes; unique_integer separates cases.
    Pid = list_to_binary(os:getpid()),
    Id = integer_to_binary(erlang:unique_integer([positive, monotonic])),
    Address = #{family => local,
                path => <<0, "bream-timed-read-", Pid/binary, "-", Id/binary>>},
    {ok, Listener} = socket:open(local, stream),
    try
        ok = socket:bind(Listener, Address),
        ok = socket:listen(Listener, 1),
        {ok, Peer} = socket:open(local, stream),
        try
            ok = socket:connect(Peer, Address, 10000),
            {ok, Socket} = socket:accept(Listener, 10000),
            try Case(Socket, Peer)
            after ok = socket:close(Socket)
            end
        after ok = socket:close(Peer)
        end
    after ok = socket:close(Listener)
    end.
