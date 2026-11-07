import os
import subprocess
import requests

def weather(city: str, units: str = "metric") -> dict:
    """Get current weather for a city."""
    
    return {"a":1, "b":2}

def os_execute(cmd: str) -> str:
    """Executes the given cmd"""
    result = subprocess.run(
        cmd,
        shell=True,
        capture_output=True,
        text=True,
    )

    return {
        "returncode": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
    }
