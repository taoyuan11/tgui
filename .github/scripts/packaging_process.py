"""Bounded subprocesses used by the packaging checks; never leave child jobs behind."""
import os
import signal
import subprocess


def run_process(args, *, cwd=None, env=None, timeout=90, capture_output=False):
    command = list(map(str, args))
    options = {"cwd": cwd, "env": env, "text": True}
    if os.name == "nt":
        options["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
    else:
        options["start_new_session"] = True
    if capture_output:
        options.update(stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    process = subprocess.Popen(command, **options)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except BaseException:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                           capture_output=True, timeout=30, check=False)
        else:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        process.wait(timeout=30)
        raise
    result = subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
    result.check_returncode()
    return result
