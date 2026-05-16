#!/usr/bin/env python3
"""Print shell exports for Trackhound from Hermes/OpenClaw-imported secrets.

This intentionally does not print an OpenAI key or Gmail app password; provide
OPENAI_API_KEY, GMAIL_IMAP_USERNAME, and GMAIL_IMAP_PASSWORD yourself.
"""
import json
import shlex
from pathlib import Path

TRACK17_CONFIG = Path('/home/diverofdark/.hermes/secrets/17track/config.json')

def export(key: str, value: str):
    print(f"export {key}={shlex.quote(value)}")

track17 = json.loads(TRACK17_CONFIG.read_text())
export('TRACK17_SECURITY_KEY', track17['securityKey'])
