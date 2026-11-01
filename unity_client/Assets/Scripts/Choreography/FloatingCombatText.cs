using TMPro;
using UnityEngine;

namespace Heav3ns.Choreography
{
    /// <summary>
    /// Spawns and animates concurrent non-blocking world-space floating combat numbers and status banners
    /// above board slots when DamageDealt / ShieldBroken / ManaSpent events fire.
    /// </summary>
    [DisallowMultipleComponent]
    public class FloatingCombatText : MonoBehaviour
    {
        [SerializeField] private float floatVelocityY = 1.65f;
        [SerializeField] private float lifetime = 0.75f;
        [SerializeField] private TMP_Text textComponent;

        private float _elapsed;
        private Color _initialColor = Color.white;
        private Vector3 _initialScale = Vector3.one;

        public TMP_Text TextComponent => textComponent;

        public void Initialize(string text, Color color, float durationSeconds = 0.75f)
        {
            if (textComponent == null)
            {
                textComponent = GetComponent<TMP_Text>();
                if (textComponent == null)
                {
                    textComponent = gameObject.AddComponent<TextMeshPro>();
                }
            }

            textComponent.text = text;
            textComponent.color = color;
            textComponent.fontSize = 4.2f;
            textComponent.alignment = TextAlignmentOptions.Center;

            _initialColor = color;
            _initialScale = transform.localScale;
            lifetime = Mathf.Max(0.15f, durationSeconds);
            _elapsed = 0f;
        }

        private void Update()
        {
            Tick(Time.deltaTime);
        }

        public void Tick(float deltaTime)
        {
            _elapsed += deltaTime;
            transform.position += Vector3.up * (floatVelocityY * deltaTime);

            float t = Mathf.Clamp01(_elapsed / Mathf.Max(0.01f, lifetime));
            float punch = t < 0.2f ? Mathf.Lerp(1.35f, 1.0f, t / 0.2f) : 1.0f;
            transform.localScale = _initialScale * punch;

            if (textComponent != null)
            {
                Color c = _initialColor;
                c.a = Mathf.Clamp01(1f - (t * t));
                textComponent.color = c;
            }

            if (_elapsed >= lifetime)
            {
                if (Application.isPlaying)
                {
                    Destroy(gameObject);
                }
                else
                {
                    DestroyImmediate(gameObject);
                }
            }
        }

        public static FloatingCombatText Spawn(Vector3 worldPosition, string message, Color color, Transform parent = null)
        {
            GameObject go = new GameObject($"FloatingCombatText_{message}");
            if (parent != null)
            {
                go.transform.SetParent(parent, true);
            }
            go.transform.position = worldPosition + new Vector3(0f, 0.55f, 0f);
            go.transform.rotation = Quaternion.Euler(55f, 0f, 0f);

            TextMeshPro tmp = go.AddComponent<TextMeshPro>();
            RectTransform rt = go.GetComponent<RectTransform>();
            if (rt != null)
            {
                rt.sizeDelta = new Vector2(3.5f, 0.9f);
            }

            FloatingCombatText fct = go.AddComponent<FloatingCombatText>();
            fct.textComponent = tmp;
            fct.Initialize(message, color, 0.75f);
            return fct;
        }
    }
}
