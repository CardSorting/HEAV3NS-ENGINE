using System.Collections;
using Heav3ns.UI.Navigation;
using Heav3ns.Visuals;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>Physical pack and five card actors follow the live camera in the shared scene.</summary>
    public sealed class GachaAltarVisuals : MonoBehaviour
    {
        private HubNavigationRouter _router;
        private GachaSummonStage _summon;
        private Transform _root;
        private GameObject _pack;
        private CardVisual[] _cards;
        private Material _packMaterial;
        private Material[] _frontMaterials;
        private Material _backMaterial;
        private Camera _camera;
        private bool _lastUnsealed;
        private int _lastCount;
        private bool[] _flipping;
        private float[] _flipAngles;
        private int _hovered = -1;
        private AudioSource _whisperSource;
        private AudioClip[] _whispers;
        private Material _beamMaterial;

        public void Initialize(HubNavigationRouter router, GachaSummonStage summon)
        {
            _router = router; _summon = summon;
            _whisperSource = gameObject.AddComponent<AudioSource>(); _whisperSource.playOnAwake = false;
            _beamMaterial = new Material(Shader.Find("Sprites/Default"));
            _whispers = new[] { CreateWhisper(720f, false), CreateWhisper(440f, true), CreateWhisper(55f, true) };
            _camera = Camera.main;
            if (_camera == null) return;
            var root = new GameObject("Astral altar actors"); root.transform.SetParent(_camera.transform, false); _root = root.transform;
            _pack = GameObject.CreatePrimitive(PrimitiveType.Cube); _pack.name = "Floating foil pack";
            _pack.transform.SetParent(_root, false); Destroy(_pack.GetComponent<Collider>());
            _pack.transform.localPosition = new Vector3(0, 0, 1.5f); _pack.transform.localScale = new Vector3(.325f, .475f, .03f);
            Shader shader = Shader.Find("Heav3ns/AstralClockworkPortal"); if (shader == null) shader = Shader.Find("Standard");
            _packMaterial = new Material(shader) { color = new Color(.55f, .20f, .13f) }; _pack.GetComponent<Renderer>().sharedMaterial = _packMaterial;
            _packMaterial.mainTexture = Resources.Load<Texture2D>("Hub/Cards/card-back");
            _summon.Changed += Refresh;
            _router.ScreenChanged += OnScreenChanged;
            Refresh();
        }
        private void OnScreenChanged(HubScreenState from, HubScreenState to) { Refresh(); }
        private void Refresh()
        {
            if (_root == null || _router == null || _summon == null) return;
            bool visible = _router.Current == HubScreenState.GachaSummonAltar;
            _root.gameObject.SetActive(visible);
            _pack.SetActive(visible && _summon.HasPack && !_summon.IsUnsealed);
            if (_summon.IsUnsealed && _cards == null)
            {
                _cards = new CardVisual[_summon.Cards.Count];
                _flipping = new bool[_cards.Length]; _flipAngles = new float[_cards.Length];
                _frontMaterials = new Material[_cards.Length];
                Shader foil = Shader.Find("Heav3ns/CardSurfaceFoil");
                _backMaterial = new Material(foil) { mainTexture = Resources.Load<Texture2D>("Hub/Cards/card-back") };
                for (int i = 0; i < _cards.Length; i++)
                {
                    _flipAngles[i] = 180f;
                    var item = _summon.Cards[i];
                    CardVisual card = CardVisual.ConstructProceduralCardObject("Astral summon card");
                    card.transform.SetParent(_root, false); card.Collider.enabled = false;
                    card.ConfigureCard(0, item.Card.id, item.Card.deck == "Alice" ? 0 : 1, item.Card.title, item.Card.type, item.Card.cost, item.Card.text, 0, 1, true, "summon", false);
                    Transform front = card.transform.Find("FrontFace");
                    foreach (Renderer renderer in front.GetComponentsInChildren<Renderer>(true)) renderer.enabled = false;
                    var image = GameObject.CreatePrimitive(PrimitiveType.Quad); image.name = "Full card illustration";
                    Destroy(image.GetComponent<Collider>()); image.transform.SetParent(front, false);
                    image.transform.localPosition = new Vector3(0, .01f, 0); image.transform.localRotation = Quaternion.Euler(90, 0, 0);
                    image.transform.localScale = new Vector3(1.24f, 1.78f, 1);
                    _frontMaterials[i] = new Material(foil) { mainTexture = Resources.Load<Texture2D>("Hub/Cards/" + item.Card.id) };
                    image.GetComponent<Renderer>().sharedMaterial = _frontMaterials[i];
                    card.BackFaceRenderer.sharedMaterial = _backMaterial;
                    var backBlock = new MaterialPropertyBlock(); card.BackFaceRenderer.GetPropertyBlock(backBlock);
                    backBlock.SetTexture("_MainTex", _backMaterial.mainTexture); card.BackFaceRenderer.SetPropertyBlock(backBlock);
                    _cards[i] = card;
                }
            }
            if ((!_summon.HasPack || !_summon.IsUnsealed) && _cards != null)
            {
                StopAllCoroutines();
                for (int i = _root.childCount - 1; i >= 0; i--)
                    if (_root.GetChild(i).name == "Rarity light pillar") Destroy(_root.GetChild(i).gameObject);
                foreach (var card in _cards) if (card != null) Destroy(card.gameObject);
                foreach (var material in _frontMaterials) if (material != null) Destroy(material);
                if (_backMaterial != null) Destroy(_backMaterial);
                _cards = null; _flipping = null; _flipAngles = null; _hovered = -1;
            }
            if (_cards != null)
            {
                for (int i = 0; i < _cards.Length; i++)
                {
                    if (_cards[i] == null) continue;
                    bool revealed = _summon.IsRevealed(i);
                    if (_cards[i].IsFaceDown && revealed && !_flipping[i])
                    {
                        StartCoroutine(Flip(i, _cards[i]));
                    }
                }
            }
            if ((!_lastUnsealed && _summon.IsUnsealed) || (_lastCount < _summon.RevealedCount))
            {
                var audio = FindAnyObjectByType<TabletopAudioAndProps>();
                if (audio != null) audio.PlaySound(_summon.IsUnsealed && !_lastUnsealed ? TabletopAudioAndProps.SoundEffectType.ShieldShatter : TabletopAudioAndProps.SoundEffectType.CardDrawSnap, .6f);
                if (_summon.IsUnsealed && !_lastUnsealed) BurstPack();
            }
            _lastUnsealed = _summon.IsUnsealed; _lastCount = _summon.RevealedCount;
        }
        private void BurstPack()
        {
            var spring = _camera != null ? _camera.GetComponent<SecondOrderSpring>() : null;
            if (spring != null) spring.AddImpulse(new Vector3(.3f, .18f, 0));
            var go = new GameObject("Pack unseal sparks"); go.transform.SetParent(_root, false); go.transform.localPosition = new Vector3(0, 0, 1.5f);
            var sparks = go.AddComponent<ParticleSystem>();
            var main = sparks.main; main.loop = false; main.startLifetime = .55f; main.startSpeed = .25f; main.startSize = .014f; main.startColor = new Color(1, .7f, .2f, 1);
            var emission = sparks.emission; emission.rateOverTime = 0;
            var shape = sparks.shape; shape.shapeType = ParticleSystemShapeType.Sphere; shape.radius = .06f;
            sparks.GetComponent<ParticleSystemRenderer>().sharedMaterial = _beamMaterial;
            sparks.Emit(45); Destroy(go, 1.2f);
        }
        private void LateUpdate()
        {
            if (_root == null || !_root.gameObject.activeSelf) return;
            if (_pack.activeSelf) _pack.transform.localPosition = new Vector3(0, Mathf.Sin(Time.unscaledTime * 1.8f) * .02f, 1.5f);
            if (_cards == null || _camera == null) return;
            if (_hovered >= 0 && _hovered < _cards.Length && _summon.Cards[_hovered].Rarity == SummonRarity.Ssr) ApplyGlow();
            float scale = HubViewport.Scale;
            float xOffset = HubViewport.Left, yOffset = HubViewport.Top;
            for (int i = 0; i < _cards.Length; i++)
            {
                if (_cards[i] == null) continue;
                float screenX = xOffset + (220 + i * 295) * scale;
                float arc = Mathf.Abs(i - 2) * Mathf.Abs(i - 2) * 8f;
                float screenY = Screen.height - (yOffset + (500 + arc) * scale);
                Vector3 local = _camera.transform.InverseTransformPoint(_camera.ScreenToWorldPoint(new Vector3(screenX, screenY, 1.5f)));
                if (_hovered == i && !_summon.IsRevealed(i)) local.y += .03f;
                _cards[i].Spring.SetTarget(local, Quaternion.Euler(-90, 0, 0) * Quaternion.Euler(_flipAngles[i], 0, 0), Vector3.one * .225f, false);
            }
        }
        private IEnumerator Flip(int index, CardVisual card)
        {
            _flipping[index] = true;
            float elapsed = 0;
            while (elapsed < .16f && card != null)
            {
                elapsed += Time.unscaledDeltaTime; _flipAngles[index] = Mathf.Lerp(180, 90, elapsed / .16f); yield return null;
            }
            if (card != null) { card.SetFaceDown(false); card.Spring.TriggerImpactSquashPunch(); StartCoroutine(RevealLight(card, _summon.Cards[index].Rarity)); }
            elapsed = 0;
            while (elapsed < .16f && card != null)
            {
                elapsed += Time.unscaledDeltaTime; _flipAngles[index] = Mathf.Lerp(90, 0, elapsed / .16f); yield return null;
            }
            if (_flipping != null && index < _flipping.Length) { _flipAngles[index] = 0; _flipping[index] = false; }
        }
        public void SetHoveredCard(int index)
        {
            if (_hovered == index || _cards == null) return;
            _hovered = index;
            if (index >= 0 && index < _cards.Length && !_summon.IsRevealed(index) && _whisperSource != null)
                _whisperSource.PlayOneShot(_whispers[(int)_summon.Cards[index].Rarity], .4f);
            ApplyGlow();
        }
        private void ApplyGlow()
        {
            if (_cards == null) return;
            for (int i = 0; i < _cards.Length; i++)
            {
                var renderer = _cards[i] != null ? _cards[i].FrameBorderRenderer : null;
                if (renderer == null) continue;
                var block = new MaterialPropertyBlock(); renderer.GetPropertyBlock(block);
                bool active = i == _hovered && !_summon.IsRevealed(i);
                Color color = !active ? Color.black : _summon.Cards[i].Rarity == SummonRarity.Ssr ? Color.HSVToRGB(Mathf.Repeat(Time.unscaledTime * .25f, 1f), .75f, 1f) : _summon.Cards[i].Rarity == SummonRarity.SuperRare ? new Color(1, .7f, .1f) : Color.cyan;
                block.SetColor("_GlowColor", color); block.SetColor("_GlowSecondaryColor", color);
                block.SetFloat("_GlowIntensity", active ? 2f : 0f); block.SetFloat("_HighlightState", active ? 1f : 0f);
                renderer.SetPropertyBlock(block);
            }
        }
        private static AudioClip CreateWhisper(float frequency, bool chord)
        {
            const int rate = 22050; int count = rate / 2;
            var samples = new float[count];
            for (int i = 0; i < count; i++)
            {
                float t = (float)i / rate, envelope = Mathf.Sin(Mathf.PI * i / count);
                float wave = Mathf.Sin(2 * Mathf.PI * frequency * t);
                if (chord) wave = .65f * wave + .35f * Mathf.Sin(2 * Mathf.PI * frequency * 1.5f * t);
                samples[i] = wave * envelope * .18f;
            }
            AudioClip clip = AudioClip.Create("Rarity whisper", count, 1, rate, false); clip.SetData(samples, 0); return clip;
        }
        private IEnumerator RevealLight(CardVisual card, SummonRarity rarity)
        {
            var go = new GameObject("Rarity light pillar"); go.transform.SetParent(_root, false); go.transform.localPosition = card.transform.localPosition;
            var light = go.AddComponent<Light>(); light.type = LightType.Point; light.range = 2.5f;
            light.color = rarity == SummonRarity.Ssr ? Color.magenta : rarity == SummonRarity.SuperRare ? new Color(1, .7f, .1f) : Color.cyan;
            var beam = go.AddComponent<LineRenderer>(); beam.useWorldSpace = false; beam.positionCount = 2;
            beam.SetPosition(0, Vector3.zero); beam.SetPosition(1, new Vector3(0, .8f, 0)); beam.widthMultiplier = .018f;
            beam.sharedMaterial = _beamMaterial;
            for (float t = 0; t < .4f && light != null; t += Time.unscaledDeltaTime)
            {
                float fade = 1f - t / .4f; light.intensity = fade * 3f;
                Color color = light.color; color.a = fade; beam.startColor = color; color.a = 0; beam.endColor = color;
                yield return null;
            }
            if (go != null) Destroy(go);
        }
        private void OnDestroy()
        {
            if (_summon != null) _summon.Changed -= Refresh;
            if (_router != null) _router.ScreenChanged -= OnScreenChanged;
            if (_root != null) Destroy(_root.gameObject);
            if (_packMaterial != null) Destroy(_packMaterial);
            if (_beamMaterial != null) Destroy(_beamMaterial);
            if (_frontMaterials != null) foreach (var material in _frontMaterials) if (material != null) Destroy(material);
            if (_backMaterial != null) Destroy(_backMaterial);
            if (_whispers != null) foreach (var clip in _whispers) if (clip != null) Destroy(clip);
        }
    }
}
