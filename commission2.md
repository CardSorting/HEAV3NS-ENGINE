With your `unity_client` project initialized on Unity `6000.6.7f1`, the live MCP bridge running on `[http://127.0.0.1:2718](http://127.0.0.1:2718)`, the `libheav3ns_engine.dylib` FFI bridge verified, and the `EnchantedTableStage` anchors (`Alice` and `WhiteRabbit` Hand, Sequence, and Shield rows) in place inside `Battlefield.unity`, the next step is commissioning the agent to build out the actual playable card game, tactile 2.5D physics, visual eventchoreography, and HUD.

Copy and paste the commission specification below directly into your agent.

---

## Commission Specification: Complete Gameplay Loop, 2.5D Card Physics & Visual Choreography (`commission_phase2.md`)

### 1. Objective

Transform the bootstrapped `Battlefield.unity` stage and verified `RustEngineBridge` into a fully playable, tactile 2.5D card game between **Alice** and **White Rabbit**. All work must be executed via the live **Unity MCP bridge (`[http://127.0.0.1:2718](http://127.0.0.1:2718)`)** and verified with `./scripts/unity_cli.sh HeadlessCliRunner.CompileAndVerify` and automated PlayMode tests.

---

### 2. Phase 1: 3D/2.5D Card Prefab & Spring Physics (`Assets/Scripts/Visuals/`)

Build a procedural/prefab-backed physical card object that looks and feels like weighted cardboard on the `EnchantedTableStage`:

1. **`CardVisual.cs` & Prefab Generator (`HeadlessCliRunner.CreateCardPrefab`):**
* Construct a reusable `Assets/Prefabs/CardActor.prefab` featuring a 3D quad/box mesh with front/back faces, ornate frame border, artwork slot, and world-space `TextMeshPro` labels for **Cost, Attack/Power, Health/Defense, Title, and Rules Text**.
* Attach a `BoxCollider` for raycast picking.


2. **`SecondOrderSpring.cs` (Damped Spring & Tilt Engine):**
* Replace all linear interpolation with a frame-rate independent second-order spring-damper solver (`stiffness`, `damping`, `overshoot`).
* **Velocity-Driven 2.5D Tilt:** Calculate instantaneous velocity $(v_x, v_y)$ as the card moves toward its target position and apply proportional pitch/roll rotation plus dynamic shadow offset.
* **Squash & Stretch Impact:** When a card snaps into `AliceSequenceRow`, `AliceShieldRow`, or their `WhiteRabbit` counterparts, apply a 2-frame impact scale punch ($1.15\times$ X/Z, $0.85\times$ Y) before springing back to $1.0$.



---

### 3. Phase 2: Zone Layout Managers (`Assets/Scripts/Board/`)

Bind the 6 existing anchors under `EnchantedTableStage` to deterministic spatial layout controllers:

1. **`HandArcLayout.cs` (`AliceHandAnchor` & `WhiteRabbitHandAnchor`):**
* Arrange cards along a quadratic Bezier fan curve with subtle Z-axis fan rotation.
* **Hover Magnification:** When the mouse raycasts a card in `AliceHandAnchor`, elevate its Y/Z position toward the camera, flatten its rotation to $0^\circ$, scale it by $1.35\times$, and part neighboring cards horizontally so full card text is unobstructed.


2. **`BoardRowLayout.cs` (`AliceSequenceRow`, `AliceShieldRow`, `WhiteRabbitSequenceRow`, `WhiteRabbitShieldRow`):**
* Manage slot spacing, ghost-card insertion previews while dragging a playable card over a row, and subtle idle sine-wave breathing for units/cards stationed in Sequence and Shield rows.
* Enforce face-down / masked orientation for opponent hand cards and unrevealed shield cards based on `heav3ns_engine_get_board_state_json`.



---

### 4. Phase 3: Input Controller, Drag-and-Drop & Targeting Arrow (`Assets/Scripts/Input/`)

1. **`CardInteractionController.cs`:**
* Raycast from `Main Camera` against `CardVisual` colliders and `BoardSurface`.
* **Drag State Machine:** `Idle` $\rightarrow$ `Hovering` $\rightarrow$ `DraggingCard` (or `AimingTarget`) $\rightarrow$ `ReleaseCommit` / `CancelReturn`.
* **Bezier Targeting Reticle (`TargetingArrowRenderer.cs`):** When playing a targeted card or commanding a unit in `AliceSequenceRow` to attack/interact, render a curved 3D Bezier arc of animated chevron meshes from the source card to the cursor, snapping to valid targets.
* **Command Dispatch:** On valid release, serialize the corresponding action and invoke `RustEngineBridge.SubmitCommandJson(...)`, immediately queuing the resulting visual events into `VisualEventQueue`.



---

### 5. Phase 4: `VisualEventQueue` Sequencer & VFX Choreography (`Assets/Scripts/Choreography/`)

Expand `VisualEventQueue.cs` from a basic verifier into a coroutine/async presentation director that drains events emitted by `heav3ns_engine_drain_visual_events_json`:

1. **Event Handlers:**
* `CardDrawn` / `CardDealt`: Spawn `CardActor` at deck origin, flip rotation, and spring into `AliceHandAnchor` or `WhiteRabbitHandAnchor`.
* `CardPlayed` / `UnitPlaced` / `ShieldPlaced`: Arc card from hand into the designated `SequenceRow` or `ShieldRow` slot, trigger board dust particles, and apply camera recoil.
* `AttackDeclared` / `CombatResolved` / `DamageDealt`: Wind back the attacker, lunge rapidly into the defender's transform, spawn an impact particle burst, spawn a floating world-space damage number (`FloatingCombatText.cs`), and trigger trauma-based camera shake on `Main Camera`.
* `ShieldBroken` / `CardDestroyed`: Trigger a shatter/dissolve visual effect and remove the actor from its row layout.
* `TurnChanged` / `GameOver`: Display a dramatic center-screen banner and toggle interactive turn controls.


2. **Blocking vs. Non-Blocking Pacing:**
* Combat lunges, sequence triggers, and turn banners block the queue for their exact duration ($0.25\text{s}–0.5\text{s}$); floating numbers and stat-badge pulses run concurrently.



---

### 6. Phase 5: Diegetic Tabletop HUD, Opponent Pacing & Audio (`Assets/Scripts/UI/`)

1. **Diegetic Tabletop Controls:**
* Build a 3D clickable **End Turn Lever/Button** on the edge of `EnchantedTableStage/BoardSurface` with spring depression and state coloring (playable actions remaining vs. turn complete).
* Display Mana/Resource crystals, Deck counts, Graveyard/Discard piles, and Hero/Player life totals for both **Alice** and **White Rabbit** using crisp world-space UI.
* Add a **Match Reset / New Game** control on the HUD when `GameOver` is reached.


2. **White Rabbit Turn Trigger:**
* When the turn passes to `WhiteRabbit`, trigger the Rust engine's AI/opponent turn action (or automated opponent command step) and let `VisualEventQueue` choreograph White Rabbit's plays at human-readable pacing.


3. **Procedural Audio & Interactive Corners (`TabletopAudioAndProps.cs`):**
* Add a multi-layered `AudioSource` pool with pitch randomization ($\pm 4\%$) for card draw snaps, heavy cardboard board thuds, shield shatters, and button clicks.
* Add 2 clickable interactive tabletop props on the corners of `EnchantedTableStage` (e.g., an enchanted pocket watch and a teacup) that react with spring physics and particles when clicked.



---

### 7. Phase 6: End-to-End Verification & Acceptance Criteria

1. **Headless Playable Bootstrap (`HeadlessCliRunner.BuildFullBattlefield`):**
* Update `HeadlessCliRunner.cs` to generate `CardActor.prefab`, wire all managers onto `EnchantedTableStage` and `RustEngineBridgeHost`, and save `Assets/Scenes/Battlefield.unity`.


2. **Automated End-to-End Simulation (`HeadlessCliRunner.RunPlayableMatchSimulation`):**
* Write an automated verification method that initializes a match via `RustEngineBridge`, draws opening hands for Alice and White Rabbit, spawns the corresponding `CardVisual` instances into the scene anchors, simulates playing cards into `AliceSequenceRow` and `AliceShieldRow`, advances turns, drains the entire `VisualEventQueue`, and asserts that the Unity scene hierarchy and `CardVisual` states match `heav3ns_engine_get_board_state_json` with zero exceptions.


3. **Definition of Done:**
* `./scripts/unity_cli.sh HeadlessCliRunner.CompileAndVerify` exits with `0` (0 errors, 0 warnings).
* `./scripts/unity_cli.sh HeadlessCliRunner.RunPlayableMatchSimulation` passes 100% of assertions.
* Live MCP inspection confirms `Battlefield.unity` is saved and immediately playable on pressing Play in the Unity Editor.



---

For setting up clean world-space labels and dynamic card text inside your Unity 6 prefabs, this [Unity 6 TextMeshPro tutorial](https://www.youtube.com/watch?v=geRIMokPsNQ) walks through configuring text components and scripting updates in C#.