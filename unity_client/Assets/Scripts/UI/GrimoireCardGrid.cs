using Heav3ns.UI.Navigation;
using Heav3ns.Visuals;
using UnityEngine;

namespace Heav3ns.UI
{
    /// <summary>Reusable eight-card physical drawer; filters update card identity in place.</summary>
    public sealed class GrimoireCardGrid : MonoBehaviour
    {
        private readonly CardVisual[] _actors = new CardVisual[8];
        private readonly Material[] _fronts = new Material[8];
        private GrimoireDeckStudio _studio;
        private HubNavigationRouter _router;
        private Transform _root;
        private Camera _camera;
        public void Initialize(GrimoireDeckStudio studio, HubNavigationRouter router)
        {
            _studio = studio; _router = router; _camera = Camera.main;
            if (_camera == null) return;
            _root = new GameObject("Grimoire card drawer").transform; _root.SetParent(_camera.transform, false);
            for (int i = 0; i < _actors.Length; i++)
            {
                _actors[i] = CardVisual.ConstructProceduralCardObject("Grimoire card");
                _actors[i].transform.SetParent(_root, false); _actors[i].Collider.enabled = false;
                Transform front = _actors[i].transform.Find("FrontFace");
                foreach (Renderer renderer in front.GetComponentsInChildren<Renderer>(true)) renderer.enabled = false;
                var image = GameObject.CreatePrimitive(PrimitiveType.Quad); image.name = "Full card illustration";
                Destroy(image.GetComponent<Collider>()); image.transform.SetParent(front, false);
                image.transform.localPosition = new Vector3(0, .01f, 0); image.transform.localRotation = Quaternion.Euler(90, 0, 0);
                image.transform.localScale = new Vector3(1.24f, 1.78f, 1);
                _fronts[i] = new Material(Shader.Find("Heav3ns/CardSurfaceFoil")); image.GetComponent<Renderer>().sharedMaterial = _fronts[i];
            }
            _router.ScreenChanged += OnScreenChanged; _studio.Changed += Refresh;
            Refresh();
        }
        private void OnScreenChanged(HubScreenState from, HubScreenState to) { Refresh(); }
        private void Refresh()
        {
            if (_root == null) return;
            bool visible = _router.Current == HubScreenState.GrimoireStudio;
            _root.gameObject.SetActive(visible);
            if (!visible) return;
            for (int i = 0; i < _actors.Length; i++)
            {
                int index = _studio.Page * 8 + i;
                bool available = index < _studio.FilteredCards.Count;
                _actors[i].gameObject.SetActive(available);
                if (!available) continue;
                var card = _studio.FilteredCards[index];
                _actors[i].ConfigureCard(index, card.id, card.deck == "Alice" ? 0 : 1, card.title, card.type, card.cost, card.text, 0, 1, false, "collection", false);
                _fronts[i].mainTexture = Resources.Load<Texture2D>("Hub/Cards/" + card.id);
                _fronts[i].SetFloat("_FoilIntensity", _studio.FoilPreview ? 1.2f : 0f);
                var renderer = _actors[i].ArtworkSlotRenderer;
                if (renderer != null)
                {
                    var block = new MaterialPropertyBlock(); renderer.GetPropertyBlock(block);
                    block.SetFloat("_FoilIntensity", _studio.FoilPreview ? 1.2f : 0f);
                    renderer.SetPropertyBlock(block);
                }
            }
        }
        private void LateUpdate()
        {
            if (_root == null || !_root.gameObject.activeSelf) return;
            float scale = HubViewport.Scale;
            float left = HubViewport.Left, top = HubViewport.Top;
            for (int i = 0; i < _actors.Length; i++)
            {
                if (!_actors[i].gameObject.activeSelf) continue;
                float x = left + (165 + (i % 4) * 218) * scale;
                float y = Screen.height - (top + (427 + (i / 4) * 195) * scale);
                Vector3 local = _camera.transform.InverseTransformPoint(_camera.ScreenToWorldPoint(new Vector3(x, y, 1.5f)));
                _actors[i].Spring.SetTarget(local, Quaternion.Euler(-90, -8, 0), Vector3.one * .125f, false);
            }
        }
        private void OnDestroy()
        {
            if (_router != null) _router.ScreenChanged -= OnScreenChanged;
            if (_studio != null) _studio.Changed -= Refresh;
            if (_root != null) Destroy(_root.gameObject);
            foreach (var material in _fronts) if (material != null) Destroy(material);
        }
    }
}
