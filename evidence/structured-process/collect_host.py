"""Record post-cohort host observations without replaying the benchmark."""
from pathlib import Path
import hashlib
import json
import os
import platform
import sys

root = Path(__file__).resolve().parents[2]
run = root / 'evidence/structured-process/runs' / sys.argv[1]
output = run / 'host-observation.json'
assert run.is_dir() and not output.exists()
paths = ['/proc/cpuinfo', '/proc/meminfo', '/proc/loadavg',
         '/sys/devices/system/cpu/cpufreq/boost']
for cpu in [0, 1]:
    paths += [f'/sys/devices/system/cpu/cpu{cpu}/topology/{name}' for name in ['core_id', 'physical_package_id', 'thread_siblings_list']]
    paths += [f'/sys/devices/system/cpu/cpu{cpu}/cpufreq/{name}' for name in ['scaling_driver', 'scaling_governor', 'energy_performance_preference', 'scaling_cur_freq']]
data = {'when': 'After recorded cohort; instantaneous observations do not establish frequency during samples.',
        'platform': platform.platform(), 'collector_affinity': sorted(os.sched_getaffinity(0)),
        'allocator': 'Rust default system allocator; no custom global allocator in these crates.',
        'performance_counters': 'Not measured in this experiment.',
        'observations': {p: Path(p).read_text() if Path(p).exists() else None for p in paths},
        'collector': {'path': str(Path(__file__).relative_to(root)), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}}
output.write_text(json.dumps(data, indent=2)+'\n')
print(output.relative_to(root))
