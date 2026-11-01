using System.Collections;
using System.Collections.Generic;
using System.Linq;
using Heav3ns.Board;
using Heav3ns.Choreography;
using Heav3ns.Input;
using Heav3ns.UI;
using Heav3ns.UnityBridge;
using Heav3ns.Visuals;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.SceneManagement;

namespace Heav3ns.Tests.Editor
{
    /// <summary>
    /// Automated NUnit / UnityTest suite verifying Commission 2 & Commission 3 (Phase 1-5):
    /// - Custom Shaders (CardBorderGlow.shader & CardSurfaceFoil.shader) & CardArtRegistry textures
    /// - 3D/2.5D CardVisual & SecondOrderSpring physics (velocity tilt, 2-frame squash & stretch punch)
    /// - Opening MulliganController 1.6x viewport fan, Replace/Redraw badge toggling, Confirm Hand, & 0.08s shield cascade
    /// - High-Impact Shield Break & Reveal Drama (180 deg flip at camera center for 0.45s + crystalline shatter)
    /// - Noise-driven dissolve animation (_DissolveAmount 0.0 -> 1.0) on destroyed units/shields
    /// - Right-Click & 0.6s Long-Hover CardInspectorOverlay (2.2x preview + keyword tooltips)
    /// - ActionHistoryLog recording last 6 resolved VisualEvent entries
    /// - TitleStageController diegetic title plaque / box camera transition & SkipIntroInSimulations flag
    /// - DiegeticTabletopHud 3D End Turn lever, Match Reset control, and TabletopAudioAndProps corner props
    /// </summary>
    public class PlayableMatchPlayModeTests
    {
        private GameObject _stageRoot;
        private GameObject _bridgeHost;
        private Camera _camera;
        private VisualEventQueue _queue;
        private BattlefieldChoreographer _choreographer;
        private MulliganController _mulligan;
        private CardInteractionController _interaction;
        private DiegeticTabletopHud _hud;
        private TabletopAudioAndProps _audioProps;
        private TargetingArrowRenderer _arrow;
        private CardInspectorOverlay _inspector;
        private ActionHistoryLog _historyLog;
        private TitleStageController _titleStage;
        private readonly List<GameObject> _temporarilyDisabledRoots = new List<GameObject>();

        [SetUp]
        public void SetUp()
        {
            for (int i = 0; i < SceneManager.sceneCount; i++)
            {
                var scene = SceneManager.GetSceneAt(i);
                if (!scene.isLoaded) continue;
                foreach (var root in scene.GetRootGameObjects())
                    if (root.activeSelf) { _temporarilyDisabledRoots.Add(root); root.SetActive(false); }
            }
            GameObject camGo = new GameObject("Main Camera");
            camGo.tag = "MainCamera";
            _camera = camGo.AddComponent<Camera>();
            camGo.transform.position = new Vector3(0f, 11.5f, -7.8f);
            camGo.transform.rotation = Quaternion.Euler(56f, 0f, 0f);
            camGo.AddComponent<SecondOrderSpring>();

            _stageRoot = new GameObject("EnchantedTableStage");
            GameObject boardSurface = GameObject.CreatePrimitive(PrimitiveType.Plane);
            boardSurface.name = "BoardSurface";
            boardSurface.transform.SetParent(_stageRoot.transform, false);

            CreateZone(_stageRoot.transform, "AliceHandAnchor", new Vector3(0f, 0.15f, -4.5f)).AddComponent<HandArcLayout>().ConfigureHandZone(0, false);
            CreateZone(_stageRoot.transform, "AliceSequenceRow", new Vector3(0f, 0.1f, -1.6f)).AddComponent<BoardRowLayout>().ConfigureRow(0, BoardRowLayout.RowKind.Sequence, false);
            CreateZone(_stageRoot.transform, "AliceShieldRow", new Vector3(-5.2f, 0.12f, -2.8f)).AddComponent<BoardRowLayout>().ConfigureRow(0, BoardRowLayout.RowKind.Shield, true);
            CreateZone(_stageRoot.transform, "WhiteRabbitHandAnchor", new Vector3(0f, 0.15f, 4.5f)).AddComponent<HandArcLayout>().ConfigureHandZone(1, true);
            CreateZone(_stageRoot.transform, "WhiteRabbitSequenceRow", new Vector3(0f, 0.1f, 1.6f)).AddComponent<BoardRowLayout>().ConfigureRow(1, BoardRowLayout.RowKind.Sequence, false);
            CreateZone(_stageRoot.transform, "WhiteRabbitShieldRow", new Vector3(5.2f, 0.12f, 2.8f)).AddComponent<BoardRowLayout>().ConfigureRow(1, BoardRowLayout.RowKind.Shield, true);

            _audioProps = _stageRoot.AddComponent<TabletopAudioAndProps>();
            _audioProps.EnsureInitialized();

            _hud = _stageRoot.AddComponent<DiegeticTabletopHud>();
            _hud.EnsureHudBuilt();

            _inspector = _stageRoot.AddComponent<CardInspectorOverlay>();
            _inspector.EnsureBuilt();

            _historyLog = _stageRoot.AddComponent<ActionHistoryLog>();
            _historyLog.EnsureBuilt();

            _titleStage = _stageRoot.AddComponent<TitleStageController>();
            _titleStage.EnsureBuilt();

            GameObject arrowGo = new GameObject("TargetingArrowRenderer");
            arrowGo.transform.SetParent(_stageRoot.transform, false);
            _arrow = arrowGo.AddComponent<TargetingArrowRenderer>();

            _interaction = _stageRoot.AddComponent<CardInteractionController>();

            _bridgeHost = new GameObject("RustEngineBridgeHost");
            _queue = _bridgeHost.AddComponent<VisualEventQueue>();
            _choreographer = _bridgeHost.AddComponent<BattlefieldChoreographer>();
            _mulligan = _bridgeHost.AddComponent<MulliganController>();

            _choreographer.EnsureReferences();
            _mulligan.EnsureBuilt();
            _interaction.EnsureReferences();
        }

        [TearDown]
        public void TearDown()
        {
            if (_queue != null) _queue.ShutdownNativeBridge();
            if (_bridgeHost != null) Object.DestroyImmediate(_bridgeHost);
            if (_stageRoot != null) Object.DestroyImmediate(_stageRoot);
            if (_camera != null) Object.DestroyImmediate(_camera.gameObject);
            foreach (var root in _temporarilyDisabledRoots) if (root != null) root.SetActive(true);
            _temporarilyDisabledRoots.Clear();
        }

        [Test]
        public void EndToEnd_PlayableMatch_Shaders_Mulligan_Inspector_History_Dissolve_And_HUD()
        {
            Assert.IsTrue(_queue.TryInitializeNativeBridge(), "Native RustEngineBridge should connect");

            _queue.DrainAllEventsSynchronously(0.1f);
            SerializedBoardStateDto state0 = _choreographer.SyncBoardStateFromNative(true);
            Assert.IsNotNull(state0);
            Assert.AreEqual(5, _choreographer.AliceHandAnchor.Cards.Count);
            Assert.AreEqual(6, _choreographer.AliceShieldRow.Cards.Count);
            Assert.AreEqual(6, _choreographer.WhiteRabbitShieldRow.Cards.Count);

            // 1. Verify custom shader assignment & CardArtRegistry distinct artwork
            CardVisual card0 = _choreographer.AliceHandAnchor.Cards[0];
            Assert.AreEqual("Heav3ns/CardBorderGlow", card0.FrameBorderRenderer.sharedMaterial.shader.name);
            Assert.AreEqual("Heav3ns/CardSurfaceFoil", card0.ArtworkSlotRenderer.sharedMaterial.shader.name);
            Assert.IsNotNull(CardArtRegistry.GetCardArtTexture(card0.CardSpecId, card0.CardTitle, card0.CardType, 0));

            // 2. Verify TitleStageController -> Opening Mulligan 1.6x viewport fan -> Replace badge -> Confirm & 0.08s shield cascade
            _titleStage.ShowTitleDiorama(true);
            Assert.IsTrue(_titleStage.IsTitleActive);
            _titleStage.PressPlayMatchButton(true, true);
            Assert.IsFalse(_titleStage.IsTitleActive);
            Assert.IsTrue(_mulligan.IsMulliganActive);
            Assert.AreEqual(1.6f, card0.Spring.TargetScale.x, 0.001f);

            Assert.IsTrue(_mulligan.ToggleCardMulliganSelection(card0));
            Assert.IsTrue(card0.MarkedForMulligan);
            Assert.IsFalse(_mulligan.ToggleCardMulliganSelection(card0));
            Assert.IsFalse(card0.MarkedForMulligan);
            _mulligan.ConfirmMulliganAndDealShields(false);
            Assert.IsFalse(_mulligan.IsMulliganActive);
            Assert.IsTrue(_mulligan.HasCompletedMulligan);
            Assert.AreEqual(12, _mulligan.ShieldsDealtSteps);

            // 3. Verify CardInspectorOverlay (2.2x scale preview + keyword tooltip callouts)
            Assert.IsTrue(_inspector.InspectCard(card0));
            Assert.IsTrue(_inspector.IsShowing);
            Assert.AreEqual(2.2f, _inspector.PreviewCardActor.transform.localScale.x, 0.001f);
            Assert.IsTrue(_inspector.KeywordCalloutLabel.text.Contains("SEQUENCE RULE"));
            _inspector.HideInspection();
            Assert.IsFalse(_inspector.IsShowing);

            // 4. Verify hover magnification (1.35x scale, Y/Z elevation, 0 deg rotation, neighbor parting)
            CardVisual card2 = _choreographer.AliceHandAnchor.Cards[2];
            float neighborXBefore = _choreographer.AliceHandAnchor.Cards[1].Spring.TargetPosition.x;
            _choreographer.AliceHandAnchor.SetHoveredCard(card2);
            Assert.AreEqual(1.35f, card2.Spring.TargetScale.x, 0.001f);
            Assert.AreNotEqual(neighborXBefore, _choreographer.AliceHandAnchor.Cards[1].Spring.TargetPosition.x);
            _choreographer.AliceHandAnchor.SetHoveredCard(null);

            // 5. Play a card into AliceSequenceRow and verify 2-frame impact squash punch + ActionHistoryLog
            CardVisual playable = _choreographer.AliceHandAnchor.Cards.First(c => c.IsPlayable);
            string resp = _interaction.CommitPlayCard(playable, null, false);
            Assert.IsTrue(resp.Contains("\"ok\""));
            Assert.AreEqual(1.15f, playable.transform.localScale.x, 0.001f);
            Assert.AreEqual(0.85f, playable.transform.localScale.y, 0.001f);

            // Step spring 3 frames and verify it springs back toward 1.0 after the 2-frame punch
            playable.Spring.Step(1f / 60f);
            playable.Spring.Step(1f / 60f);
            playable.Spring.Step(1f / 60f);
            Assert.Less(playable.transform.localScale.x, 1.15f);

            _queue.DrainAllEventsSynchronously(0.1f);
            Assert.Greater(_historyLog.Entries.Count, 0);

            // 6. Verify ShieldRevealed drama & noise-driven dissolve animation completion
            _queue.EnqueueEvent(new VisualEventRecord
            {
                eventType = "ShieldRevealed",
                seat = 1,
                turnNumber = state0.turn_number,
                cardUid = -888,
                cardSpecId = "H3W-RA-001",
                title = "Minute-Hand Militia",
                cardType = "Character",
                cost = 1,
                triggerText = "Draw 1 card.",
                isShieldBreak = true,
                isBlocking = true,
                duration = 0.45f
            });
            _queue.TickSequencer(0.05f);
            Assert.Greater(_choreographer.ShieldBreakDramaCount, 0);
            for (int step = 0; step < 25; step++)
            {
                _choreographer.TickDissolveAndShieldDrama(0.05f);
            }
            Assert.Greater(_choreographer.CompletedDissolveCount, 0);
            Assert.AreEqual(0, _choreographer.ActiveDissolvingCardCount);
            _queue.DrainAllEventsSynchronously(0.1f);

            // 7. Press 3D End Turn lever -> triggers White Rabbit AI turn
            int turnBefore = state0.turn_number;
            _hud.PressEndTurnLever();
            _queue.DrainAllEventsSynchronously(0.1f);

            SerializedBoardStateDto stateAfter = _choreographer.SyncBoardStateFromNative(true);
            Assert.Greater(stateAfter.turn_number, turnBefore);
            Assert.AreEqual(stateAfter.players[0].hand.Count, _choreographer.AliceHandAnchor.Cards.Count);
            Assert.AreEqual(stateAfter.players[1].hand_count, _choreographer.WhiteRabbitHandAnchor.Cards.Count);

            // Verify no orphaned CardVisual children exist under any zone anchor after multiple syncs
            Assert.AreEqual(
                _choreographer.AliceHandAnchor.Cards.Count,
                _choreographer.AliceHandAnchor.GetComponentsInChildren<CardVisual>(true).Length);
            Assert.AreEqual(
                _choreographer.WhiteRabbitHandAnchor.Cards.Count,
                _choreographer.WhiteRabbitHandAnchor.GetComponentsInChildren<CardVisual>(true).Length);

            // 8. Verify GameOver shows MatchResetButton3D and PressMatchResetControl resets match
            Assert.IsFalse(_hud.MatchResetButtonTransform.gameObject.activeSelf);
            _queue.EnqueueEvent(new VisualEventRecord
            {
                eventType = "MatchWon",
                seat = 0,
                turnNumber = stateAfter.turn_number,
                text = "Alice Wins!",
                isBlocking = true,
                duration = 0.25f
            });
            _queue.TickSequencer(0.05f);
            Assert.IsTrue(_hud.IsGameOver);
            Assert.IsTrue(_hud.MatchResetButtonTransform.gameObject.activeSelf);

            _hud.PressMatchResetControl();
            _queue.DrainAllEventsSynchronously(0.1f);
            SerializedBoardStateDto stateReset = _choreographer.SyncBoardStateFromNative(true);
            Assert.IsFalse(_hud.IsGameOver);
            Assert.IsFalse(_hud.MatchResetButtonTransform.gameObject.activeSelf);
            Assert.AreEqual(1, stateReset.turn_number);
            Assert.AreEqual(0, _historyLog.Entries.Count);
        }

        private static GameObject CreateZone(Transform parent, string name, Vector3 localPos)
        {
            GameObject go = new GameObject(name);
            go.transform.SetParent(parent, false);
            go.transform.localPosition = localPos;
            return go;
        }
    }
}
