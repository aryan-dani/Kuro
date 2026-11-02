"""Windows Auto-Start Manager for Kuro.

Adds or removes a shortcut in the Windows Startup folder so Kuro launches
silently in the background whenever your machine boots up.
"""

import os
import sys
import argparse
import subprocess

def get_startup_dir() -> str:
    appdata = os.environ.get("APPDATA")
    if not appdata:
        raise RuntimeError("Could not determine APPDATA environment path.")
    return os.path.join(appdata, r"Microsoft\Windows\Start Menu\Programs\Startup")

def get_shortcut_path() -> str:
    return os.path.join(get_startup_dir(), "Kuro.lnk")

def enable_autostart():
    kuro_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    vbs_script = os.path.abspath(os.path.join(os.path.dirname(__file__), "start_kuro_silent.vbs"))
    shortcut_path = get_shortcut_path()

    print(f"Configuring Windows Startup shortcut: {shortcut_path}")
    print(f"Target launcher: {vbs_script}")

    ps_code = f"""
$s = (New-Object -COM WScript.Shell).CreateShortcut('{shortcut_path}')
$s.TargetPath = 'wscript.exe'
$s.Arguments = '"{vbs_script}"'
$s.WorkingDirectory = '{kuro_dir}'
$s.Save()
"""
    res = subprocess.run(["powershell", "-NoProfile", "-Command", ps_code], capture_output=True, text=True)
    if os.path.exists(shortcut_path):
        print("\n[SUCCESS] Startup shortcut created (Wi-Fi watcher via start_kuro_silent.vbs).")
        print("Kuro starts only on home Wi-Fi — set %LOCALAPPDATA%\\Kuro\\home_ssid.txt first.")
        print("Prefer: powershell -File scripts\\Install-KuroAutostart.ps1 -HomeSsid YourWifi -StartNow")
    else:
        print(f"\n[ERROR] Failed to create startup shortcut: {res.stderr}")

def disable_autostart():
    shortcut_path = get_shortcut_path()
    if os.path.exists(shortcut_path):
        try:
            os.remove(shortcut_path)
            print("[SUCCESS] Auto-start disabled. Kuro shortcut removed from Windows Startup.")
        except Exception as e:
            print(f"[ERROR] Could not remove shortcut: {e}")
    else:
        print("[INFO] Auto-start is not currently enabled.")

def status():
    shortcut_path = get_shortcut_path()
    if os.path.exists(shortcut_path):
        print("[STATUS] Auto-start is ENABLED. Kuro will launch on Windows startup.")
    else:
        print("[STATUS] Auto-start is DISABLED.")

def main():
    parser = argparse.ArgumentParser(description="Manage Kuro Windows Auto-Start")
    parser.add_argument("--enable", action="store_true", help="Enable auto-start on Windows boot")
    parser.add_argument("--disable", action="store_true", help="Disable auto-start on Windows boot")
    parser.add_argument("--status", action="store_true", help="Check auto-start status")

    args = parser.parse_args()

    if args.enable:
        enable_autostart()
    elif args.disable:
        disable_autostart()
    elif args.status:
        status()
    else:
        print("Current status:")
        status()
        print("\nUsage:\n  python scripts/setup_autostart.py --enable\n  python scripts/setup_autostart.py --disable")

if __name__ == "__main__":
    main()
