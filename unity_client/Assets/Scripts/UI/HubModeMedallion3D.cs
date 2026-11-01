using Heav3ns.UI.Navigation;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>Clockwork command medallion in the physical box, shared with the battle CTA.</summary>
    public sealed class HubModeMedallion3D : MonoBehaviour
    {
        private HubNavigationRouter _router;
        private Transform _dial;
        private Material _metal, _crystal;
        public void Initialize(Transform parent, HubNavigationRouter router)
        {
            _router = router;
            var go = new GameObject("Wonderland duel medallion"); _dial = go.transform;
            _dial.SetParent(parent, false); _dial.localPosition = new Vector3(0, .48f, 0); _dial.localRotation = Quaternion.Euler(0, -8, 0);
            Shader shader = Shader.Find("Heav3ns/AstralClockworkPortal"); if (shader == null) shader = Shader.Find("Standard");
            _metal = new Material(shader) { color = new Color(.75f, .50f, .18f) };
            _crystal = new Material(Shader.Find("Standard")) { color = new Color(.26f, .09f, .16f) };
            Part(PrimitiveType.Cylinder, "Brass bezel", Vector3.zero, new Vector3(.82f, .08f, .82f), _metal);
            Part(PrimitiveType.Cylinder, "Ruby dial", new Vector3(0, .11f, 0), new Vector3(.62f, .055f, .62f), _crystal);
            Part(PrimitiveType.Cube, "Clock hand", new Vector3(0, .18f, .2f), new Vector3(.045f, .035f, .42f), _metal);
            _router.ScreenChanged += OnScreenChanged; OnScreenChanged(router.Current, router.Current);
        }
        private void Part(PrimitiveType type, string name, Vector3 position, Vector3 scale, Material material)
        {
            var part = GameObject.CreatePrimitive(type); part.name = name; part.transform.SetParent(_dial, false);
            part.transform.localPosition = position; part.transform.localScale = scale;
            Destroy(part.GetComponent<Collider>()); part.GetComponent<Renderer>().sharedMaterial = material;
        }
        private void OnScreenChanged(HubScreenState from, HubScreenState to) { if (_dial != null) _dial.gameObject.SetActive(to != HubScreenState.ActiveBattlefield); }
        private void Update() { if (_dial != null) _dial.Rotate(0, 16f * Time.unscaledDeltaTime, 0, Space.Self); }
        private void OnDestroy()
        {
            if (_router != null) _router.ScreenChanged -= OnScreenChanged;
            if (_dial != null) Destroy(_dial.gameObject);
            if (_metal != null) Destroy(_metal); if (_crystal != null) Destroy(_crystal);
        }
    }
}
