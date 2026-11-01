using Heav3ns.Choreography;
using Heav3ns.UI.Navigation;
using Heav3ns.UnityBridge;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>Validates and starts the selected loadout before changing screens.</summary>
    public sealed class PreBattleDrawerController : MonoBehaviour
    {
        private HubProgress _progress;
        private HubNavigationRouter _router;
        private VisualEventQueue _queue;
        private BattlefieldChoreographer _battle;
        public string Error { get; private set; }
        public void Initialize(HubProgress progress, HubNavigationRouter router, VisualEventQueue queue, BattlefieldChoreographer battle)
        { _progress = progress; _router = router; _queue = queue; _battle = battle; }
        public bool Commence(int storyStage)
        {
            Error = null;
            if (_router == null || _router.IsTransitioning || _router.Current != HubScreenState.PreBattleDrawer) return false;
            if (_battle == null || _queue == null) { Error = "Battlefield is unavailable."; return false; }
            if (_progress.ActiveDeck.Count != 30) { Error = "Complete your 30-card deck in the Grimoire."; return false; }
            int selectedStage = Mathf.Clamp(storyStage, 0, 3);
            if (!_queue.StartCustomDeck(_progress.ActiveDeck, (ulong)(42 + selectedStage), (uint)selectedStage)) { Error = "The deck is invalid or the Rust engine could not start."; return false; }
            var hud = FindAnyObjectByType<DiegeticTabletopHud>();
            if (hud != null) hud.SetPlayerNames(_progress.Data.whiteRabbit ? "White Rabbit" : "Alice", _progress.Data.whiteRabbit ? "Alice" : "White Rabbit");
            _battle.AdoptInitializedMatch();
            return _router.NavigateTo(HubScreenState.ActiveBattlefield);
        }
    }
}
