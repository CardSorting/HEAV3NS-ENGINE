using System;
using Heav3ns.UI.Navigation;
using UnityEngine;

namespace Heav3ns.UI
{
    public sealed class HeroCharacterShowcase : MonoBehaviour
    {
        private HubProgress _progress;
        private int _line;
        private float _bounce;
        private static readonly string[] AliceLines = { "The sky is only another door.", "A careful sequence can change the ending.", "Shall we see what the cards remember?", "Keep a key for the unexpected." };
        private static readonly string[] RabbitLines = { "No time to waste, your move.", "Every second is a little victory.", "A quick opening can turn the tide.", "The clock is on our side." };
        public string Dialogue => (_progress.Data.whiteRabbit ? RabbitLines : AliceLines)[_line % 4];
        public int LineIndex => _line % 4;
        public float Bounce => _bounce;
        public Shader StandeeShader => Shader.Find("Heav3ns/AnimeStandeeBreathing");
        public Texture2D Portrait => Resources.Load<Texture2D>(_progress.Data.whiteRabbit ? "Hub/Characters/WhiteRabbit" : "Hub/Characters/Alice");
        public void Initialize(HubProgress progress) { _progress = progress ?? throw new ArgumentNullException(nameof(progress)); }
        public void React()
        {
            _line++; _bounce = 1f;
            var audio = FindAnyObjectByType<TabletopAudioAndProps>();
            if (audio != null) audio.PlaySound(TabletopAudioAndProps.SoundEffectType.PropChime, .65f);
        }
        public void SwitchLeader() { _progress.SetLeader(!_progress.Data.whiteRabbit); _line = 0; }
        private void Update() { _bounce = Mathf.MoveTowards(_bounce, 0, Time.unscaledDeltaTime * 3f); }
    }
}
