"""
game_client.py — Authentication and WebSocket connection to Siege Perilous.
"""

import asyncio
import json
import ssl
import requests
import websockets
import os


class GameClient:
    def __init__(self, auth_url: str, ws_url: str, account: str, password: str):
        self.auth_url = auth_url
        self.ws_url = ws_url
        self.account = account
        self.password = password
        self.session_cookie: str | None = None
        self.player_id: int | None = None
        self.ws = None
        self._recv_queue: asyncio.Queue = asyncio.Queue()
        self._recv_task = None

    def authenticate(self):
        """POST /auth, extract session cookie and player_id."""
        resp = requests.post(
            self.auth_url,
            json={"account_name": self.account, "password": self.password},
            verify=False,  # self-signed cert
            timeout=10,
        )
        resp.raise_for_status()
        data = resp.json()
        self.player_id = data["player_id"]

        # Extract session value from Set-Cookie header
        raw_cookie = resp.headers.get("Set-Cookie", "")
        for part in raw_cookie.split(";"):
            part = part.strip()
            if part.startswith("session="):
                self.session_cookie = part.split("=", 1)[1]
                break

        if not self.session_cookie:
            raise RuntimeError("No session cookie in auth response")

        print(f"[auth] player_id={self.player_id} session={self.session_cookie[:12]}...")

    async def connect(self):
        """Open WebSocket with session cookie header."""
        ssl_ctx = ssl.create_default_context()
        ssl_ctx.check_hostname = False
        ssl_ctx.verify_mode = ssl.CERT_NONE

        headers = {"Cookie": f"session={self.session_cookie}"}
        self.ws = await websockets.connect(
            self.ws_url,
            ssl=ssl_ctx,
            additional_headers=headers,
        )
        print(f"[ws] connected to {self.ws_url}")

        # Start background receiver
        self._recv_task = asyncio.create_task(self._receiver())

    async def _receiver(self):
        """Continuously read messages from WebSocket into queue."""
        try:
            async for raw in self.ws:
                try:
                    msg = json.loads(raw)
                    await self._recv_queue.put(msg)
                except json.JSONDecodeError:
                    print(f"[ws] non-JSON message: {raw[:80]}")
        except websockets.exceptions.ConnectionClosedOK:
            print("[ws] connection closed normally")
        except Exception as e:
            print(f"[ws] receiver error: {e}")

    async def send(self, cmd: dict):
        """Send a command dict as JSON."""
        raw = json.dumps(cmd)
        await self.ws.send(raw)

    async def recv(self, timeout: float = 5.0) -> dict | None:
        """Receive one packet from the queue."""
        try:
            return await asyncio.wait_for(self._recv_queue.get(), timeout=timeout)
        except asyncio.TimeoutError:
            return None

    async def recv_all(self, duration: float = 0.5) -> list[dict]:
        """Collect all packets available within `duration` seconds."""
        packets = []
        deadline = asyncio.get_event_loop().time() + duration
        while True:
            remaining = deadline - asyncio.get_event_loop().time()
            if remaining <= 0:
                break
            try:
                pkt = await asyncio.wait_for(self._recv_queue.get(), timeout=remaining)
                packets.append(pkt)
            except asyncio.TimeoutError:
                break
        return packets

    async def close(self):
        if self._recv_task:
            self._recv_task.cancel()
        if self.ws:
            await self.ws.close()
