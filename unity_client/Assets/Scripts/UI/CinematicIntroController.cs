using System.Collections;
using Heav3ns.UI.Navigation;
using Heav3ns.Visuals;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>A physical lid and counter-rotating rings kept in the same tabletop scene.</summary>
    public sealed class CinematicIntroController : MonoBehaviour
    {
        private Transform _leftDoor, _rightDoor, _innerRing, _outerRing;
        private SecondOrderSpring _leftSpring, _rightSpring;
        private HubNavigationRouter _router;
        private Material _brass, _wood;
        private Material _emberMaterial;
        private Light _sealLight;
        private GameObject _embers;
        public void Initialize(Transform parent, HubNavigationRouter router)
        {
            _router = router;
            Shader shader = Shader.Find("Heav3ns/AstralClockworkPortal");
            if (shader == null) shader = Shader.Find("Standard");
            _brass = new Material(shader) { color = new Color(.65f, .42f, .12f) };
            _wood = new Material(Shader.Find("Standard")) { color = new Color(.17f, .045f, .055f) };
            _leftDoor = Door(parent, "Left clockwork door", new Vector3(-2.7f, .22f, 0), new Vector3(1.35f, 0, 0), out _leftSpring);
            _rightDoor = Door(parent, "Right clockwork door", new Vector3(2.7f, .22f, 0), new Vector3(-1.35f, 0, 0), out _rightSpring);
            _innerRing = Ring(parent, "Inner astrolabe", 1.0f, .38f);
            _outerRing = Ring(parent, "Outer astrolabe", 1.35f, .42f);
            var emberObject = new GameObject("Drifting mana embers"); _embers = emberObject; emberObject.transform.SetParent(parent, false); emberObject.transform.localPosition = new Vector3(0, .5f, 0);
            var particles = emberObject.AddComponent<ParticleSystem>();
            var main = particles.main; main.startLifetime = 3f; main.startSpeed = .18f; main.startSize = .035f; main.startColor = new Color(1f, .7f, .25f, .75f); main.maxParticles = 80;
            var emission = particles.emission; emission.rateOverTime = 12f;
            var shape = particles.shape; shape.shapeType = ParticleSystemShapeType.Sphere; shape.radius = 1.6f;
            var renderer = particles.GetComponent<ParticleSystemRenderer>();
            _emberMaterial = new Material(Shader.Find("Sprites/Default")); renderer.sharedMaterial = _emberMaterial;
            var seal = new GameObject("Clockwork seal light"); seal.transform.SetParent(parent, false); seal.transform.localPosition = new Vector3(0, 1, 0);
            _sealLight = seal.AddComponent<Light>(); _sealLight.type = LightType.Point; _sealLight.color = new Color(1, .65f, .25f); _sealLight.range = 6; _sealLight.intensity = 0;
            router.ScreenChanged += ChangeScreen;
        }
        private Transform Door(Transform parent, string name, Vector3 pivot, Vector3 offset, out SecondOrderSpring spring)
        {
            var go = new GameObject(name); go.transform.SetParent(parent, false); go.transform.localPosition = pivot;
            spring = go.AddComponent<SecondOrderSpring>(); spring.SetTarget(pivot, Quaternion.identity, Vector3.one, true);
            Cube(go.transform, "Velvet door", offset, new Vector3(2.65f, .10f, 3.6f), _wood);
            Cube(go.transform, "Brass stile", offset + new Vector3(0, .07f, 1.72f), new Vector3(2.65f, .06f, .08f), _brass);
            Cube(go.transform, "Brass lower stile", offset + new Vector3(0, .07f, -1.72f), new Vector3(2.65f, .06f, .08f), _brass);
            Cube(go.transform, "Brass clasp", offset + new Vector3(offset.x > 0 ? 1.16f : -1.16f, .12f, 0), new Vector3(.24f, .16f, .42f), _brass);
            return go.transform;
        }
        private static void Cube(Transform parent, string name, Vector3 position, Vector3 scale, Material material)
        {
            GameObject go = GameObject.CreatePrimitive(PrimitiveType.Cube); go.name = name;
            go.transform.SetParent(parent, false); go.transform.localPosition = position; go.transform.localScale = scale;
            Destroy(go.GetComponent<Collider>()); go.GetComponent<Renderer>().sharedMaterial = material;
        }
        private Transform Ring(Transform parent, string name, float radius, float height)
        {
            var go = new GameObject(name); go.transform.SetParent(parent, false); go.transform.localPosition = new Vector3(0, height, 0);
            var line = go.AddComponent<LineRenderer>(); line.useWorldSpace = false; line.loop = true; line.positionCount = 96;
            line.widthMultiplier = .035f; line.sharedMaterial = _brass;
            for (int i = 0; i < line.positionCount; i++) { float angle = i * Mathf.PI * 2f / line.positionCount; line.SetPosition(i, new Vector3(Mathf.Cos(angle) * radius, 0, Mathf.Sin(angle) * radius)); }
            return go.transform;
        }
        private void ChangeScreen(HubScreenState from, HubScreenState to)
        {
            bool closed = to == HubScreenState.SplashScreen;
            if (from == HubScreenState.SplashScreen && to == HubScreenState.HubLobby)
            {
                var audio = FindAnyObjectByType<TabletopAudioAndProps>();
                if (audio != null) audio.PlaySound(TabletopAudioAndProps.SoundEffectType.PropChime);
                StopAllCoroutines(); StartCoroutine(FlashSeal());
            }
            _leftSpring.SetTarget(new Vector3(-2.7f, .22f, 0), Quaternion.Euler(0, 0, closed ? 0 : 175), Vector3.one, false);
            _rightSpring.SetTarget(new Vector3(2.7f, .22f, 0), Quaternion.Euler(0, 0, closed ? 0 : -175), Vector3.one, false);
        }
        private IEnumerator FlashSeal()
        {
            for (float t = 0; t < .5f && _sealLight != null; t += Time.unscaledDeltaTime)
            { _sealLight.intensity = Mathf.Sin(Mathf.PI * t / .5f) * 5f; yield return null; }
            if (_sealLight != null) _sealLight.intensity = 0;
        }
        private void Update()
        {
            if (_innerRing != null) _innerRing.Rotate(0, 14f * Time.unscaledDeltaTime, 0, Space.Self);
            if (_outerRing != null) _outerRing.Rotate(0, -9f * Time.unscaledDeltaTime, 0, Space.Self);
        }
        private void OnDestroy()
        {
            if (_router != null) _router.ScreenChanged -= ChangeScreen;
            if (_leftDoor != null) Destroy(_leftDoor.gameObject);
            if (_rightDoor != null) Destroy(_rightDoor.gameObject);
            if (_innerRing != null) Destroy(_innerRing.gameObject);
            if (_outerRing != null) Destroy(_outerRing.gameObject);
            if (_embers != null) Destroy(_embers);
            if (_sealLight != null) Destroy(_sealLight.gameObject);
            if (_brass != null) Destroy(_brass);
            if (_wood != null) Destroy(_wood);
            if (_emberMaterial != null) Destroy(_emberMaterial);
        }
    }
}
