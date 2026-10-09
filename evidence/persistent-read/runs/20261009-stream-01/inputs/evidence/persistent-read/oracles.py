"""Finite semantic oracles; no execution, clocks, or performance quantities."""

def matrix():
    rows = []
    for mode in [1, 2, 3]:
        for reason in [1, 2, 3, 4, 5]:
            for trap in [0, 1]:
                fatal = mode == 1 or (mode == 2 and (trap == 0 or reason == 5)) or (mode == 3 and (reason == 5 or (trap == 0 and reason != 1)))
                outcome = 2 if fatal else 1 if trap else 0
                observed = (2 if reason == 5 and mode != 1 else reason) if fatal else reason if trap else 0
                rows.append([mode, reason, trap, outcome, observed, int(mode == 3 or not fatal)])
    return rows


def cleanup():
    rows = []
    for mode, reason, trap, outcome, _, _ in matrix():
        stage = (10 if mode == 3 else 20) if outcome == 2 else 30
        rows.append([mode, reason, trap, stage, 1, 0 if mode == 3 else 19,
                     1 if mode == 3 else 3, 2 if mode == 3 else 21,
                     2 if mode == 3 else 21, 1])
    return rows


def reference():
    fields = ['mode', 'reason', 'trap', 'outcome', 'observed_reason', 'returned']
    rows = [{'kind': 'matrix', **dict(zip(fields, row))} for row in matrix()]
    for direct in [0, 1]:
        for trap in [0, 1]:
            code = 2 if direct else 5
            rows.append({'kind': 'chain', 'direct_kill': direct, 'trap': trap,
                         'source_reason': code, 'middle_reason': code,
                         'outcome': 1 if trap else 2, 'observed_reason': code})
    rows += [{'kind': 'catch', 'mode': 1, 'caught': 1, 'observed_reason': 5},
             {'kind': 'catch', 'mode': 2, 'caught': 0, 'observed_reason': 2},
             {'kind': 'dead_target', 'returned_count': 5, 'target_alive': 0},
             {'kind': 'ordering', 'trap': 0, 'same_sender_order': 1},
             {'kind': 'ordering', 'trap': 1, 'same_sender_order': 1}]
    return rows


def check_exit(row):
    assert row['kind'] == 'exit_controls' and row['passed'] and row['speed_tests'] is False
    assert row['matrix'] == matrix()
    assert row['cleanup'] == cleanup()
    assert row['busy'] == [[3, 3, 3, 10, 1, 19, 3], [5, 2, 2, 10, 1, 19, 3]]
    assert row['stale_target_ignored'] is True


def check_read(row):
    assert row['kind'] == 'read_controls' and row['passed'] and row['speed_tests'] is False
    assert row['completions'] == [[1,62629,1,0,1,1]]*2
    assert row['stops'] == [[0,2,2,0,19,3,1,1],[1,2,1,0,19,3,1,1],[2,2,3,1,19,3,1,1]]
    assert row['eof_result'] == 0 and row['unavailable_result'] == 90
    assert row['capacity_rejected'] and row['stale_isolated'] and row['peer_progress']
    assert row['shutdown_settled'] == 2 and row['read_kill_races'] == 16
    assert 0 <= row['race_deliveries'] <= 16


def check_stream(row):
    assert row['kind'] == 'stream_controls' and row['passed'] and row['speed_tests'] is False
    assert row['completed'] == [[3,3,3,0,3,3,0,1,1]]*2
    assert row['request_ids'] == [1,2]
    assert row['stops'] == [[0,1,0,0,0,0,1,19,3],[1,2,1,0,1,1,1,19,3],[2,3,1,1,0,1,1,19,3],[3,4,2,1,1,2,1,19,3],[4,5,2,2,0,2,1,19,3],[5,6,3,2,1,3,1,19,3]]
    assert row['shutdown'] == [[0,0,0,0,0,0,0,1,1],[1,1,0,1,1,1,0,1,1],[2,2,1,1,2,2,0,1,1]]
    assert row['capacity_reused'] and row['stale_isolated'] and row['peer_progress']
    assert row['second_read_kill_races'] == 16
    assert 0 <= row['race_second_deliveries'] <= 16
