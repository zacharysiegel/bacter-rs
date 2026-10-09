# Type naming convention

Source of truth for how Rust types are named and laid out across the bacter-rs codebase. Memory files reference this document; they don't restate it.

The type-naming rules below are strict: the same kind of shape always gets the same suffix (`SerialOut`, `SerialIn`, `Serial`, `Kind`). Drift here makes `shared/src/protocol/` unreadable.

The variable-naming guidance at the end is softer: a default to fall back to, not a rule to enforce in code review.

## Underlying heuristic: hierarchical naming

The rules below all share one underlying preference: hierarchical naming over English-sounding naming. Most-significant noun first; modifiers and qualifiers after. Concretely for types:

- The model name comes first; the wire-shape suffix follows: `MemberSerialOut`, not `SerialOutMember`.
- Direction comes before redaction context: `MemberSerialOutPublic`, not `PublicMemberSerialOut`. Every same-direction variant shares the `MemberSerialOut...` prefix.
- Enum disambiguators trail the noun: `GameModeKind`, not `KindOfGameMode`.

Why: items sharing a hierarchical axis sort and grep together. `grep "MemberSerialOut"` finds every outbound variant of `Member`; an alphabetical listing puts every model's wire types adjacent. English-sounding orderings scatter related types across the namespace.

## Core dichotomy: model and wire type

Every type that crosses the WebSocket or is written to a replay file has two shapes:

- Model: what application code uses (simulation, server, client session). Bare-named (`GameState`, `Member`, `Loadout`). Never derives bitcode.
- Wire type: what bitcode encodes. Suffixed by direction (below). Derives `bitcode::Encode` and `bitcode::Decode`; wire types are the only types that do.

The two shapes are always two distinct structs, even when their fields are identical. The wire types mark the shapes that cannot change casually: changing a wire type changes the bytes on the wire and in replay files, so it bumps `PROTOCOL_VERSION`.

Wire types live in `shared/src/protocol/`, not in a `<feature>_model.rs`. No module outside `shared/src/protocol/`, and no other crate, names `bitcode`.

## Wire formats

Direction is the primary axis. Redaction is a secondary modifier and composes onto direction:

- `<Model>SerialOut`: server to client. Replay files are server output, so their records use `SerialOut` types, the header included (`ReplayHeaderSerialOut`).
- `<Model>SerialIn`: client to server. Often differs from `Out` because the client doesn't supply server-assigned fields.
- `<Model>Serial`: only when In and Out are byte-identical. Don't reach for this just to save a struct; if there's any chance In and Out will diverge later, write them separately from the start.
- `<Model>SerialOutPublic` and similar: context modifiers for redaction, suffixed after the direction. Don't introduce these speculatively; wait until there's a real second consumer.
- Wire-only types without a model (message envelopes such as `MessageSerialIn` and `MessageSerialOut`) have no conversions.
- Newtypes and bit sets are primitives on the wire.

## Conversion impl placement

- In the wire type's file in `shared/src/protocol/`, immediately after the wire type.
- Sending side: `impl From<&Model> for <Model>SerialOut` (or `<Model>Serial`), always infallible.
- Client input: `impl TryFrom<<Model>SerialIn> for Model` (client input can fail validation).
- Receiving side of server output (client decoding, replay reading): `impl TryFrom<<Model>SerialOut> for Model` or `impl TryFrom<<Model>Serial> for Model`. `From` only for plain enum mirrors, where nothing can fail.
- Receiving: `bitcode::decode` into the wire type, then `TryFrom` into the model. Sending: `From` into the wire type, then `bitcode::encode`.
- A model whose invariants sit in private fields owns them in its own module: raw-part accessors for the `From`, and a validating constructor returning `Option` which the `TryFrom` in `protocol/` calls.

## Enums

Two flavors:

- `<Noun>Kind`: when the bare name would shadow or be confused for a related struct. Without the suffix, a reader seeing `GameMode` reasonably assumes a data-bearing struct; the `Kind` suffix is the signal that this is an enumeration of variant tags. Use `Kind` whenever a related struct of the bare name exists, or when the bare name would otherwise read as a noun describing a thing rather than a classification.
- Bare descriptive name: when the type name already unambiguously reads as an enumeration of values: `RoundPhase`, `AbilityPhase`. The `Phase` suffix is doing the work `Kind` would.

Method naming for the string direction:

- `<field>()`: when the enum maps cleanly to a single named code whose name reads naturally as a method: `GameModeKind::code()` (`"ffa"`, `"skm"`, `"srv"`).
- `as_str()`: fallback for everything else.

Always implement `TryFrom<&str>` for the string to enum direction. This keeps the wire to model idiom uniform: `Model::try_from(wire)` works whether `wire` is a wire type or a `&str`. Don't implement `FromStr` instead: the `parse::<T>()` shortcut isn't load-bearing here, and having two near-equivalent traits just splits the codebase's idiom in half.

## Variable naming (guidance, not strict)

When in doubt, name variables after the type they hold, lowercase, plural for collections. The wire suffix is part of the variable name:

```rust
let member_serial_out: MemberSerialOut = MemberSerialOut::from(&member);
let member_serial_outs: Vec<MemberSerialOut> = members.iter().map(MemberSerialOut::from).collect();
```

Type naming is the load-bearing part of this spec; variable naming is preference. Don't burn review time on variable-rename churn.

## Database types

Deferred until a database exists: the `Entity` (table-row mirror) and `Projection` (join or subset) wire types, their conversions, and `query_scalar!` for single-column queries follow eafora's `docs/conventions/types.md` when persistence is added.

## Where bacter-rs diverges from Singularity

- Encoding: bitcode with its own `Encode`/`Decode` derive, in place of serde and rmp-serde. No serde until a consumer exists.
- Direction: Singularity uses `Serial` for outbound and `PublicSerial` for the redacted variant; bacter-rs uses the `SerialIn`/`SerialOut` split above, which is more honest about why two structs exist when they do.
