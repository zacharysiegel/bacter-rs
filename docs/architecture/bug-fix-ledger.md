# Bug-fix ledger

- Quoted entries are bug titles of the original (branches `a3.2.4` and `master`); unquoted entries describe bugs without a title.

## Moot: the Node server is replaced
Typed registry, one task per game owning its timers, handlers returning `Result`, membership keyed by `MemberId`.
- "'org' handler references undefined `host`"
- "spawn/kill call functions that do not exist on Games"; "the server join, respawn, die and game-ended handlers call functions that do not exist"
- "kill() ignores its `player` argument and splices index -1"; "kill() ignores its `player` argument and has no guard for 'not found'"
- "spawn() removes the spectator entry twice"
- "removeMember is called without a host and uses the nonexistent games.get"; "removeMember host branch calls Map.remove and assumes a shrink interval exists"; "removeMember non-host branch: out-of-scope variable, wrong spectator removed, team lookup after splice, missing config.colors"; "Player leaving a team game dereferences config.colors (missing) and the org it just spliced"
- "Games.remove() uses this.games and leaks intervals and timeouts"; "Host pressing Back (or ESC) in its own join menu crashes the server"
- "'end round' references undefined `s`"; "Round timeouts are never cancelled when a game is removed and crash when they fire"; "Pre-round force-spawn timeout uses a stale index and is never cleared when the host leaves"
- "Per-game broadcast interval captures a stale array index"
- "createGame overwrites an existing game for the same host and leaks its interval"
- "Object.fromEntries does not exist on the declared Node 9.4.0 runtime"
- "Socket handlers throw on a missing acknowledgement callback or malformed payload"; "Malformed payloads crash the whole server because handlers do no validation" (decode errors close one connection, [failure isolation](./server.md#failure-isolation))
- "Leaving and spectating rely on broken server handlers"

## Wire shape mismatches
Shared `MessageSerialIn`/`MessageSerialOut` enums; a mismatch does not compile.
- "Ability.js emits 'ability' with the raw Ability object"; "The 'ability' event is sent with the wrong shape"; "'ability' handler never finds the player"
- "Org update payload field names do not match the server handler"

## Client authority and trust
Clients send only intents and inputs; the simulation adjudicates; the server validates membership.
- "Any client can overwrite any game's board, teams, flag, rounds and its own org without validation"; "Client-authoritative org, kill and board state with whole-list overwrites"
- "Leaderboard updates are last-writer-wins on whole-list overwrites"; "Leaderboard is replaced wholesale from stale client copies, dropping members"; "Joining or respawning overwrites the server's leaderboard and teams with a stale client snapshot"
- "'join spectator' mutates the client's copy of the game"
- "Damage detection runs on the victim's client, sampled at timer rate"
- "Player cap enforced only on the client against a stale snapshot" (checked in the game task)
- "Join validation reads a stale global org from a previous game" (server validation; per-session client state)
- "'game ended' trusts the client-sent title" (no end-game message exists)

## Passwords
Hash in `GameHandle`, verified inside the join request.
- "Password protection is enforced only on the client"
- "Any client can replace any game's password" (set only in `CreateGame`)
- "'ask permission' crashes when the game has no security entry, and calls the callback twice" (one reply per request)
- "Security entry is never deleted on game end"; "Host-ended games never remove their password" (dies with the handle)
- Plaintext storage and per-keystroke transmission (bcrypt; password sent once on submit)

## Rooms, lobby, bandwidth
- "Game titles double as room names without server-side uniqueness"; "Duplicate-title check never runs, and games share socket.io rooms by title"; "Duplicate-title check never runs; titles are socket.io room names, so games share broadcasts (including the 'lobby' room)"; "Game titles are used as room names without server-side uniqueness checks": routing by `GameId`, atomic title reservation.
- "A new 'games' broadcast interval is created on every connection and never cleared"; "A new games-broadcast interval is created on every socket connection": one lobby task.
- "The full game state is broadcast every 40 ms per game, and every game to every socket each second"; "The network protocol sends full state at high frequency": bundles and summaries ([protocol](./protocol.md)).
- "Game browser crashes whenever any game exists": typed `GameList`, keyed rows.
- "Team list in the join menu calls Game.games.map.get": team sizes from `GameSummary.team_sizes`.

## Timing and frame-rate dependence
- "Movement and spore advancement depend on network packet arrival"; "Movement and projectile speed depend on server broadcast delivery": per-frame crosshair at 42.5 px/s, per-tick projectiles.
- "Pending ability timers survive respawn and title transitions": tick deadlines in state, reset on spawn.
- "Round countdown subtracts a server timestamp from the client clock": countdown from ticks.
- "Client event listeners are re-registered on every reconnect"; "Socket listeners are registered again on every reconnect"; "Client registers all socket listeners again on every reconnect": one socket at a time, no automatic reconnect.
- Cooldowns re-enabled only from the HUD render path: expiry in step phase 3.
- Doubled-interval guard: moot.

## Rounds
Round state machine inside `step`.
- "Round progression gets stuck in several cases"; "Round progression depends on one particular client's live tick"
- "Pre-round start and cancel conditions use different counts": one participant predicate.
- "'end round' and the win can be applied more than once": transition happens once in `Playing`.
- "forceSpawn turns pure spectators into players with an undefined color"; "Spectators who joined from the spectate menu are force-spawned with an undefined color": only Participants with a loadout are force spawned.
- "Survival world position is not restored between rounds": whole `initial_bounds` restored.
- Survival winner required `players[0]` to be the local client: survivor detected in `step`.

## Simulation rule bugs fixed
- "Shooting crashes when the aim angle is NaN": zero aim is a no-op; integer selection.
- "Shoot target selection ignores angle wraparound": cosine maximisation.
- "Shoot-secretion neutralize check indexes cells with the wrong loop variable": one `is_protected_by_neutralize(cell)`.
- "Spawn-position overlap check stops after the first org": all organisms and hazards checked; attempt limit.
- Rectangle spawn retry sets `repos = false`: moot (candidates are generated inside the bounds).
- "World-boundary tests are wrong for offset ellipse worlds and inconsistent for rectangles": unified rule, real centre.
- "Natural death recomputes the O(n²) regions scan after every removal": dense bitmap, one pass per phase.
- `removeCell` fallthrough removing `cells[0]`: removal by lattice coordinate.
- Friendly-fire guard testing an undefined global (`Org.js:427`): moot; team check is explicit.
- Own effects reaching growth one round trip late (server echo): effects apply in the same tick.
- Two clients disagreeing about collisions and damage: single deterministic simulation.

## Kept as deferred rules
- Rules of [deferred questionable rules](./simulation.md#deferred-questionable-rules).
- "Spore or shoot on a small org removes its last cells and kills it" (rule 7).
- "Adjacent birth spots are not deduplicated, so birth odds scale with neighbor count" (rule 4).
- "Kill credit comes from a stale or friendly 'hit'": staleness kept (rule 2); teammate hits no longer record; neutralized hits never recorded (faithful).
- Spore at the centroid vanishing (NaN direction): kept, as a dropped spore with no projectile ([abilities](./simulation.md#abilities)).
- Self damage from own acid (rule 1); neutralize protecting everyone (rule 5); cursor-centred fields (rule 6); teammates blocking growth (rule 3).

## UI and menus
- "renderNeutralize ignores its parameter": every player's neutralize drawn.
- "Spectator respawning into a team game crashes because org.team is undefined": team from `TeamChoiceKind`, validated by the server.
- "Screen-name uniqueness check loops over the wrong bound": server checks all members.
- "Cap-versus-minimum check is skipped when player minimum is left at its default": shared validation; cross-field rules apply only to fields the mode has ([limits](./protocol.md#limits)).
- "Join-menu team preselection does not pick the smallest team": argmin preselection.
- "Leaderboard spectator-skip logic hides players and can index past the end of the list": rows derived by filter and take.
- "Game-closed paths return to a title screen with no background animation": one return-to-title transition.
- "Tutorial bot is constructed with arguments in the wrong order"; "Tutorial respawn check tests the global player org instead of each org"; "Leaving the tutorial does not clear its pending task timeouts": [tutorial and title](./client-web.md#tutorial-and-title).
- Leaderboard sort reading the global mode instead of the board's: rows derived from `GameState.settings.mode` ([scoring and leaderboard](./simulation.md#scoring-and-leaderboard)).
- Respawn team-balance check nested inside the team loop (`submit.js:545`): one server-side check ([lifecycle](./server.md#lifecycle)).
- Name-label truncation testing 30 characters but cutting to 20 (`HUD.js:98-101`): rule kept as displayed ([overlays](./client-web.md#overlays)), centring fixed to the shown text.
- `Board.find` ignoring its `id`; player minimum and team count coupling inverted; errors keyed to missing fields; `Message.width` by character count: fixed in the rewritten views.
- "Apply in the Infection pause menu sets org.color to undefined"; "Capture the Flag flag can never be picked up"; tag never ending; ctf 700 px minimum: moot (modes not implemented).
- "getMpos is undefined": moot (aim comes from the input layer).
- `alert()` usage: message box and inline errors.
