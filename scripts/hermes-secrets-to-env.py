#!/usr/bin/env python3
"""Print shell exports for Trackhound from Hermes/OpenClaw-imported secrets.

This intentionally does not print an OpenAI key; provide OPENAI_API_KEY yourself.
"""
import json
import shlex
from pathlib import Path

GMAIL_CREDENTIALS = Path('/home/diverofdark/.hermes/secrets/gmail/credentials.json')
GMAIL_TOKEN = Path('/home/diverofdark/.hermes/secrets/gmail/token.json')
TRACK17_CONFIG = Path('/home/diverofdark/.hermes/secrets/17track/config.json')

def export(key: str, value: str):
    print(f"export {key}={shlex.quote(value)}")

creds = json.loads(GMAIL_CREDENTIALS.read_text())
installed = creds.get('installed') or creds.get('web') or {}
token = json.loads(GMAIL_TOKEN.read_text())
track17 = json.loads(TRACK17_CONFIG.read_text())

export('GMAIL_CLIENT_ID', installed['client_id'])
export('GMAIL_CLIENT_SECRET', installed['client_secret'])
export('GMAIL_REFRESH_TOKEN', token['refresh_token'])
export('GMAIL_TOKEN_URI', token.get('token_uri') or installed.get('token_uri') or 'https://oauth2.googleapis.com/token')
export('TRACK17_SECURITY_KEY', track17['securityKey'])
