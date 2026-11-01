using UnityEngine;

namespace Heav3ns.UI.Navigation
{
    /// <summary>Letterboxed 1600x900 design coordinates inside the device safe area.</summary>
    public static class HubViewport
    {
        public static float Scale { get { Rect safe = Screen.safeArea; return Mathf.Min(safe.width / 1600f, safe.height / 900f); } }
        public static float Left { get { Rect safe = Screen.safeArea; return safe.x + (safe.width - 1600f * Scale) / 2f; } }
        public static float Top { get { Rect safe = Screen.safeArea; return Screen.height - safe.yMax + (safe.height - 900f * Scale) / 2f; } }
    }
}
