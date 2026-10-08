"""Historical validator superseded after user-requested speed-evidence removal."""
from pathlib import Path
import runpy
runpy.run_path(str(Path(__file__).resolve().parents[2] / 'evidence/performance-reset/validate.py'), run_name='__main__')
