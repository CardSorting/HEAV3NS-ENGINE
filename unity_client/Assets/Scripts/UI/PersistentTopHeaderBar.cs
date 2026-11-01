using System;
using Heav3ns.UI.Navigation;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>Header data shared across hub modules. Tea regenerates every five minutes.</summary>
    public sealed class PersistentTopHeaderBar : MonoBehaviour
    {
        public const int MaxTea = 120;
        public const int TeaRegenSeconds = 300;
        private HubProgress _progress;
        private HubNavigationRouter _router;
        public int Tea => _progress.Data.tea;
        public string RegenLabel
        {
            get
            {
                if (Tea >= MaxTea) return "FULL";
                long now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
                long seconds = Math.Max(0, _progress.Data.teaUpdatedAt + TeaRegenSeconds - now);
                return TimeSpan.FromSeconds(seconds).ToString(@"mm\:ss");
            }
        }
        public bool ProfileVisible => _router.Current == HubScreenState.HubLobby;
        public void Initialize(HubProgress progress, HubNavigationRouter router) { _progress = progress; _router = router; _progress.RefreshTea(DateTimeOffset.UtcNow.ToUnixTimeSeconds()); }
        private float _nextRefresh;
        private void Update()
        {
            if (_progress == null || Time.unscaledTime < _nextRefresh) return;
            _nextRefresh = Time.unscaledTime + 1f;
            _progress.RefreshTea(DateTimeOffset.UtcNow.ToUnixTimeSeconds());
        }
    }
}
