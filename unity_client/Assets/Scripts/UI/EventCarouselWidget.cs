using Heav3ns.UI.Navigation;
using UnityEngine;

namespace Heav3ns.UI
{
    public sealed class EventCarouselWidget : MonoBehaviour
    {
        private HubNavigationRouter _router;
        private float _elapsed;
        public int Index { get; private set; }
        public string Title => Index == 0 ? "RATE UP  //  CHECKMATE THE SKY" : "ACT I  //  THE MAD TEA PARTY";
        public void Initialize(HubNavigationRouter router) { _router = router; }
        public void Activate()
        {
            if (_router != null) _router.NavigateTo(Index == 0 ? HubScreenState.GachaSummonAltar : HubScreenState.PreBattleDrawer);
        }
        private void Update()
        {
            if (_router == null || _router.Current != HubScreenState.HubLobby) return;
            _elapsed += Time.unscaledDeltaTime;
            if (_elapsed >= 6f) { _elapsed = 0; Index = (Index + 1) % 2; }
        }
    }
}
