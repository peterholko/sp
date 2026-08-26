# Admin dashboard

The read-only operations dashboard is available at `/admin` on the Axum web
service. It is intended for the small alpha environment and reports:

- authoritative live game connections;
- Online, Safe Logout Pending, Offline Protected, and Disconnected presence;
- hero identity, class, position, state, activity, and health;
- Safe Logout countdown/protected duration and the latest cancellation or
  rejection reason;
- the player's current personal-crisis kind, phase, and pressure; and
- non-sensitive account identity and player state.

Ordinary disconnected runtime records are hidden by default. They can be shown
from the dashboard when diagnosing a stale run. The page refreshes every five
seconds and does not contain gameplay controls.

## Access

Both `/admin` and `/admin/api/status` require an unexpired HttpOnly `session`
cookie belonging to an account whose `accounts.is_admin` value is explicitly
`TRUE`. A missing or `NULL` flag is denied. For a development account, grant
access directly in PostgreSQL:

```sql
UPDATE accounts SET is_admin = TRUE WHERE account_name = 'YourAccountName';
```

No email address, password hash, session credential, trusted-device credential,
or WebSocket connection UUID is returned by the dashboard API.

## Runtime boundary

Axum cannot directly read the Bevy ECS world. The game thread therefore
publishes a small immutable in-memory snapshot. Axum requests that snapshot over
the existing TLS WebSocket listener using the admin's current session.

The game server independently validates the session expiry and `is_admin` flag.
An admin-status socket returns before it creates a gameplay `Client` or `Stream`,
so dashboard polling cannot replace, disconnect, or control the admin's active
game connection.

The dashboard and API use no-store, frame-denial, referrer, and content-security
headers. The status request is read-only and performs no simulation or account
mutation beyond the normal throttled session-activity refresh.
