#!/usr/bin/env python3
"""Finite SC protocol abstractions, not a BEAM emulator or a Rust proof.

Explore every reachable state of two deliberately small ownership models.
Deliberately broken variants must produce counterexamples. See report 33
for source correspondence, atomicity assumptions and excluded behavior.
"""
from collections import deque
from dataclasses import dataclass, replace, asdict
from pathlib import Path
import hashlib
import json
import platform
import sys


@dataclass(frozen=True)
class Retention:
    reader: int = 0
    exit: int = 0
    guard: bool = False
    found: bool = False
    pinned: bool = False
    published: bool = True
    body: bool = True
    baseline: bool = True
    record: bool = True


def retention_edges(s, variant):
    actions = ['enter', 'lookup', 'pin', 'leave', 'inspect', 'release']
    if variant == 'release_guard_before_pin':
        actions[2:4] = ['leave', 'pin']
    if s.reader < len(actions):
        action = actions[s.reader]
        n = replace(s, reader=s.reader + 1)
        fault = None
        if action == 'enter':
            n = replace(n, guard=True)
        elif action == 'lookup':
            n = replace(n, found=s.published)
            if s.published and not s.record:
                fault = 'lookup_dereferences_freed_record'
        elif action == 'pin' and s.found:
            if not s.record:
                fault = 'increment_reference_after_record_free'
            n = replace(n, pinned=True)
        elif action == 'leave':
            n = replace(n, guard=False)
        elif action == 'inspect' and s.found:
            if not s.record:
                fault = 'retained_metadata_access_after_free'
            elif variant == 'reference_authorizes_heap_access' and not s.body:
                fault = 'heap_access_after_body_reclamation'
        elif action == 'release':
            n = replace(n, pinned=False)
        if not n.baseline and not n.pinned:
            n = replace(n, record=False)
        yield 'reader.' + action, n, fault

    if s.exit == 0:
        yield 'exit.unpublish', replace(s, exit=1, published=False), None
    elif s.exit == 1:
        yield 'exit.reclaim_body', replace(s, exit=2, body=False), None
    elif s.exit == 2 and (not s.guard or variant == 'skip_grace_period'):
        n = replace(s, exit=3, baseline=False, record=s.pinned)
        fault = ('borrowed_record_freed_inside_guard'
                 if s.guard and s.found and not n.record else None)
        yield 'exit.release_baseline_after_grace', n, fault


@dataclass(frozen=True)
class Parking:
    receiver: int = 0
    sender: int = 0
    lock: str = ''
    fresh_input: bool = False
    receiver_observed: bool = False
    active: bool = True
    running: bool = True
    reserved: bool = False
    queued: bool = False
    enqueue_owner: str = ''
    snapshot: tuple = ()


def word(s):
    return (s.active, s.running, s.reserved)


def parking_edges(s, variant):
    # Only direct incoming publication. No buffered ingress, timeout, exit,
    # priority/proxy entry, migration or scan of old unmatched messages.
    r = s.receiver
    if r == 0 and not s.lock:
        yield 'receiver.lock_MSGQ', replace(s, receiver=1, lock='receiver'), None
    elif r == 1:
        if s.fresh_input:
            yield ('receiver.final_fetch_sees_input',
                   replace(s, receiver=8, receiver_observed=True, lock=''), None)
        else:
            yield 'receiver.final_fetch_empty', replace(s, receiver=2), None
    elif r == 2:
        if variant == 'clear_active_after_unlock':
            yield 'receiver.unlock_too_early', replace(s, receiver=3, lock=''), None
        else:
            yield 'receiver.clear_ACTIVE_under_MSGQ', replace(s, receiver=3, active=False), None
    elif r == 3:
        if variant == 'clear_active_after_unlock':
            yield 'receiver.clear_ACTIVE_too_late', replace(s, receiver=4, active=False), None
        else:
            yield 'receiver.unlock_MSGQ', replace(s, receiver=4, lock=''), None
    elif r == 4:
        yield 'receiver.load_schedule_state', replace(s, receiver=5, snapshot=word(s)), None
    elif r == 5:
        if word(s) != s.snapshot and variant != 'blind_schedule_out_store':
            yield 'receiver.CAS_retry', replace(s, receiver=4, snapshot=()), None
        else:
            active, _, reserved = s.snapshot
            needs_queue = active and not reserved
            n = replace(s, receiver=6, active=active, running=False,
                        reserved=reserved or needs_queue,
                        enqueue_owner='receiver' if needs_queue else s.enqueue_owner,
                        snapshot=())
            yield 'receiver.commit_schedule_out', n, None
    elif r == 6:
        n = replace(s, receiver=8)
        if s.enqueue_owner == 'receiver':
            n = replace(n, queued=True, enqueue_owner='')
        yield 'receiver.publish_reserved_queue_entry_or_finish', n, None

    p = s.sender
    if p == 0 and not s.lock:
        yield 'sender.lock_MSGQ', replace(s, sender=1, lock='sender'), None
    elif p == 1:
        yield 'sender.append_fresh_input', replace(s, sender=2, fresh_input=True), None
    elif p == 2:
        yield 'sender.unlock_MSGQ', replace(s, sender=3, lock=''), None
    elif p == 3:
        # Abstract the notification CAS loop into its successful atomic update.
        # It can run after the receiver has already fetched the message.
        n = replace(s, sender=4)
        if not s.active:
            enqueue = not s.running and not s.reserved
            n = replace(n, active=True, reserved=s.reserved or enqueue,
                        enqueue_owner='sender' if enqueue else s.enqueue_owner)
        yield 'sender.notify_activate_and_reserve_if_needed', n, None
    elif p == 4:
        n = replace(s, sender=5)
        if s.enqueue_owner == 'sender':
            n = replace(n, queued=True, enqueue_owner='')
        yield 'sender.publish_reserved_queue_entry_or_finish', n, None


def parking_invariant(s):
    if s.queued and (s.running or not s.reserved):
        return 'invalid_physical_queue_ownership'
    if s.reserved and not (s.queued or s.enqueue_owner):
        return 'orphaned_enqueue_reservation'
    if s.receiver == 8 and s.sender == 5:
        if (s.fresh_input and not s.receiver_observed
                and not s.running and not s.queued):
            return 'lost_wakeup_with_unexamined_input_and_no_pending_notification'
    return None


def path(parents, s):
    out = []
    while parents[s] is not None:
        previous, event = parents[s]
        out.append({'event': event, 'state': asdict(s)})
        s = previous
    return list(reversed(out))


def explore(kind, variant):
    initial = Retention() if kind == 'retention' else Parking()
    edges = retention_edges if kind == 'retention' else parking_edges
    parents = {initial: None}
    todo = deque([initial])
    transitions = 0
    terminals = 0
    witness = None
    while todo:
        s = todo.popleft()
        if (kind == 'retention' and s.pinned and not s.body
                and s.record and witness is None):
            witness = path(parents, s)
        successors = list(edges(s, variant))
        if not successors:
            terminals += 1
            if kind == 'retention':
                assert s.reader == 6 and s.exit == 3 and not s.record and not s.body
            else:
                assert s.receiver == 8 and s.sender == 5
        for event, n, fault in successors:
            transitions += 1
            if kind == 'parking':
                fault = fault or parking_invariant(n)
            if fault:
                return {'kind': kind, 'variant': variant, 'result': 'counterexample',
                        'search_exhausted': False, 'states_seen': len(parents),
                        'transitions_examined': transitions, 'fault': fault,
                        'trace': path(parents, s) + [{'event': event, 'state': asdict(n)}]}
            if n not in parents:
                parents[n] = (s, event)
                todo.append(n)
    return {'kind': kind, 'variant': variant, 'result': 'no_counterexample_in_model',
            'search_exhausted': True, 'states_seen': len(parents),
            'transitions_examined': transitions, 'terminal_states': terminals,
            'retained_record_after_body_reclamation_witness': witness}


def main():
    variants = [('retention', 'correct'),
                ('retention', 'release_guard_before_pin'),
                ('retention', 'skip_grace_period'),
                ('retention', 'reference_authorizes_heap_access'),
                ('parking', 'correct'),
                ('parking', 'clear_active_after_unlock'),
                ('parking', 'blind_schedule_out_store')]
    results = [explore(*args) for args in variants]
    verified = all((x['result'] == 'no_counterexample_in_model') == (x['variant'] == 'correct')
                   for x in results)
    here = Path(__file__).resolve()
    record = {'scope': 'Finite sequentially consistent abstractions; not upstream execution, weak-memory verification, Rust soundness or runtime implementation.',
              'python': sys.version, 'platform': platform.platform(),
              'source_reference': 'ad05823719d77c8faee87348ea39513d4e2f99c5',
              'script_sha256': hashlib.sha256(here.read_bytes()).hexdigest(),
              'expected_controls_observed': verified, 'results': results}
    (here.parent / 'model-results.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps({'expected_controls_observed': verified,
                      'results': [{k: v for k, v in x.items() if k not in
                                   ('trace', 'retained_record_after_body_reclamation_witness')}
                                  for x in results]}, indent=2))
    raise SystemExit(0 if verified else 1)


if __name__ == '__main__':
    main()
