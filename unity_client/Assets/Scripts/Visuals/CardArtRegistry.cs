using System;
using System.Collections.Generic;
using UnityEngine;

namespace Heav3ns.Visuals
{
    /// <summary>
    /// Procedural & Asset-Backed Card Illustration Generator & Material Cache.
    /// Maps card IDs/archetypes from the Rust engine (e.g., Checkmate the Sky, Alice, White Rabbit cards)
    /// to distinct thematic procedural emblems/textures or loaded sprites in Assets/Resources/CardArt/
    /// so every card in hand and on the board is visually distinguishable at a glance.
    /// Also caches shared Materials for CardBorderGlow.shader and CardSurfaceFoil.shader to avoid material leaks.
    /// </summary>
    public static class CardArtRegistry
    {
        private const string BorderGlowShaderName = "Heav3ns/CardBorderGlow";
        private const string SurfaceFoilShaderName = "Heav3ns/CardSurfaceFoil";

        private static readonly Dictionary<string, Texture2D> _artCache = new Dictionary<string, Texture2D>(StringComparer.OrdinalIgnoreCase);
        private static Texture2D _tabletopSurfaceTexture;
        private static Texture2D _cardBackTexture;
        private static Material _sharedBorderGlowMaterial;
        private static Material _sharedSurfaceFoilMaterial;
        private static Material _sharedTabletopMaterial;
        private static Material _sharedCardBackMaterial;

        public static Shader GetBorderGlowShader()
        {
            Shader s = Shader.Find(BorderGlowShaderName);
            if (s == null) s = Shader.Find("Standard");
            if (s == null) s = Shader.Find("Sprites/Default");
            return s;
        }

        public static Shader GetSurfaceFoilShader()
        {
            Shader s = Shader.Find(SurfaceFoilShaderName);
            if (s == null) s = Shader.Find("Standard");
            if (s == null) s = Shader.Find("Sprites/Default");
            return s;
        }

        public static Material GetSharedBorderGlowMaterial()
        {
            if (_sharedBorderGlowMaterial == null || _sharedBorderGlowMaterial.shader == null || _sharedBorderGlowMaterial.shader.name != BorderGlowShaderName)
            {
                Shader s = GetBorderGlowShader();
                if (s != null)
                {
                    _sharedBorderGlowMaterial = new Material(s)
                    {
                        name = "SharedCardBorderGlowMat",
                        enableInstancing = true
                    };
                }
            }
            return _sharedBorderGlowMaterial;
        }

        public static Material GetSharedSurfaceFoilMaterial()
        {
            if (_sharedSurfaceFoilMaterial == null || _sharedSurfaceFoilMaterial.shader == null || _sharedSurfaceFoilMaterial.shader.name != SurfaceFoilShaderName)
            {
                Shader s = GetSurfaceFoilShader();
                if (s != null)
                {
                    _sharedSurfaceFoilMaterial = new Material(s)
                    {
                        name = "SharedCardSurfaceFoilMat",
                        enableInstancing = true
                    };
                }
            }
            return _sharedSurfaceFoilMaterial;
        }

        public static Material GetSharedTabletopMaterial()
        {
            if (_sharedTabletopMaterial == null)
            {
                Shader s = GetBorderGlowShader();
                if (s != null)
                {
                    _sharedTabletopMaterial = new Material(s)
                    {
                        name = "EnchantedTabletopSurfaceMat",
                        enableInstancing = true
                    };
                    _sharedTabletopMaterial.mainTexture = GetOrCreateTabletopSurfaceTexture();
                    _sharedTabletopMaterial.SetColor("_BaseColor", new Color(0.88f, 0.85f, 0.94f, 1f));
                    _sharedTabletopMaterial.SetColor("_Color", new Color(0.88f, 0.85f, 0.94f, 1f));
                }
            }
            return _sharedTabletopMaterial;
        }

        public static Material GetSharedCardBackMaterial()
        {
            if (_sharedCardBackMaterial == null)
            {
                Shader s = GetSurfaceFoilShader();
                if (s != null)
                {
                    _sharedCardBackMaterial = new Material(s)
                    {
                        name = "WonderlandCardBackMat",
                        enableInstancing = true
                    };
                    _sharedCardBackMaterial.mainTexture = GetOrCreateCardBackTexture();
                    _sharedCardBackMaterial.SetColor("_BaseColor", Color.white);
                    _sharedCardBackMaterial.SetColor("_Color", Color.white);
                }
            }
            return _sharedCardBackMaterial;
        }

        /// <summary>
        /// Resolves a distinct artwork Texture2D for the given cardSpecId, title, cardType, and ownerSeat.
        /// Checks Assets/Resources/CardArt/{specId} first, then synthesizes a high-contrast thematic
        /// Wonderland emblem texture (Keyhole, Crown, PocketWatch, Raven, Chess Rook, Teacup, Hourglass, Rose)
        /// uniquely seeded per cardSpecId.
        /// </summary>
        public static Texture2D GetCardArtTexture(string cardSpecId, string cardTitle, string cardType, int ownerSeat)
        {
            string key = !string.IsNullOrWhiteSpace(cardSpecId)
                ? cardSpecId.Trim()
                : (!string.IsNullOrWhiteSpace(cardTitle) ? cardTitle.Trim() : $"Seat{ownerSeat}_{cardType}");

            if (_artCache.TryGetValue(key, out Texture2D cached) && cached != null)
            {
                return cached;
            }

            // 1. Try loading from Assets/Resources/CardArt/<cardSpecId>
            if (!string.IsNullOrWhiteSpace(cardSpecId))
            {
                Texture2D resourceTex = Resources.Load<Texture2D>($"CardArt/{cardSpecId.Trim()}");
                if (resourceTex != null)
                {
                    _artCache[key] = resourceTex;
                    return resourceTex;
                }

                Sprite resourceSprite = Resources.Load<Sprite>($"CardArt/{cardSpecId.Trim()}");
                if (resourceSprite != null && resourceSprite.texture != null)
                {
                    _artCache[key] = resourceSprite.texture;
                    return resourceSprite.texture;
                }
            }

            // 2. Generate distinct procedural Wonderland card illustration
            Texture2D generated = GenerateProceduralCardIllustration(key, cardTitle ?? string.Empty, cardType ?? "Character", ownerSeat);
            _artCache[key] = generated;
            return generated;
        }

        public static Texture2D GetOrCreateTabletopSurfaceTexture()
        {
            if (_tabletopSurfaceTexture != null)
            {
                return _tabletopSurfaceTexture;
            }

            const int width = 128;
            const int height = 128;
            Texture2D tex = new Texture2D(width, height, TextureFormat.RGBA32, false)
            {
                name = "EnchantedTabletopTexture",
                wrapMode = TextureWrapMode.Clamp,
                filterMode = FilterMode.Bilinear
            };

            Color32[] pixels = new Color32[width * height];
            Color deepVelvet = new Color(0.11f, 0.09f, 0.17f, 1f);
            Color centerFelt = new Color(0.16f, 0.14f, 0.25f, 1f);
            Color brassInlay = new Color(0.76f, 0.62f, 0.28f, 1f);

            for (int y = 0; y < height; y++)
            {
                float v = (y + 0.5f) / height;
                for (int x = 0; x < width; x++)
                {
                    float u = (x + 0.5f) / width;
                    float dx = (u - 0.5f) * 2f;
                    float dy = (v - 0.5f) * 2f;
                    float radial = Mathf.Sqrt(dx * dx + dy * dy);

                    Color col = Color.Lerp(centerFelt, deepVelvet, Mathf.Clamp01(radial * 0.85f));

                    // Ornate brass border and center sequence line
                    bool isOuterBorder = Mathf.Abs(dx) > 0.90f || Mathf.Abs(dy) > 0.88f;
                    bool isInnerInlay = (Mathf.Abs(Mathf.Abs(dx) - 0.84f) < 0.018f && Mathf.Abs(dy) <= 0.84f) ||
                                        (Mathf.Abs(Mathf.Abs(dy) - 0.82f) < 0.018f && Mathf.Abs(dx) <= 0.84f);
                    bool isMidfieldDivider = Mathf.Abs(dy) < 0.012f && Mathf.Abs(dx) < 0.78f;
                    bool isCenterCompass = Mathf.Abs(radial - 0.24f) < 0.014f;

                    if (isOuterBorder)
                    {
                        col = Color.Lerp(col, brassInlay * 0.65f, 0.7f);
                    }
                    else if (isInnerInlay || isMidfieldDivider || isCenterCompass)
                    {
                        col = Color.Lerp(col, brassInlay, 0.65f);
                    }

                    pixels[y * width + x] = col;
                }
            }

            tex.SetPixels32(pixels);
            tex.Apply(false, false);
            _tabletopSurfaceTexture = tex;
            return tex;
        }

        public static Texture2D GetOrCreateCardBackTexture()
        {
            if (_cardBackTexture != null)
            {
                return _cardBackTexture;
            }

            const int size = 64;
            Texture2D tex = new Texture2D(size, size, TextureFormat.RGBA32, false)
            {
                name = "WonderlandCardBackTex",
                wrapMode = TextureWrapMode.Clamp,
                filterMode = FilterMode.Bilinear
            };

            Color32[] pixels = new Color32[size * size];
            Color bg = new Color(0.20f, 0.12f, 0.32f, 1f);
            Color filigree = new Color(0.88f, 0.72f, 0.32f, 1f);
            Color inner = new Color(0.32f, 0.18f, 0.46f, 1f);

            for (int y = 0; y < size; y++)
            {
                float v = (y + 0.5f) / size * 2f - 1f;
                for (int x = 0; x < size; x++)
                {
                    float u = (x + 0.5f) / size * 2f - 1f;
                    float r = Mathf.Sqrt(u * u + v * v);
                    float angle = Mathf.Atan2(v, u);
                    float gear = Mathf.Sin(angle * 8f) * 0.04f;

                    Color c = Color.Lerp(inner, bg, Mathf.Clamp01(r));
                    if (Mathf.Abs(r - (0.55f + gear)) < 0.05f || Mathf.Abs(Mathf.Abs(u) - 0.82f) < 0.04f || Mathf.Abs(Mathf.Abs(v) - 0.82f) < 0.04f)
                    {
                        c = filigree;
                    }
                    pixels[y * size + x] = c;
                }
            }

            tex.SetPixels32(pixels);
            tex.Apply(false, false);
            _cardBackTexture = tex;
            return tex;
        }

        private static Texture2D GenerateProceduralCardIllustration(string key, string title, string cardType, int ownerSeat)
        {
            const int size = 64;
            Texture2D tex = new Texture2D(size, size, TextureFormat.RGBA32, false)
            {
                name = $"CardArt_{key}",
                wrapMode = TextureWrapMode.Clamp,
                filterMode = FilterMode.Bilinear
            };

            uint hash = ComputeDeterministicHash(key + "|" + title);
            int emblemShape = (int)(hash % 6U); // 0=Keyhole/Door, 1=Clock/Watch, 2=Crown/Sky, 3=Diamond/Spade, 4=Crescent/Smile, 5=CrossedSwords/Tower

            // Override specific iconic cards by title/id keywords
            string lowerTitle = (title ?? string.Empty).ToLowerInvariant();
            if (lowerTitle.Contains("door") || lowerTitle.Contains("key") || key.Contains("AL-001"))
            {
                emblemShape = 0;
            }
            else if (lowerTitle.Contains("watch") || lowerTitle.Contains("time") || lowerTitle.Contains("minute") || lowerTitle.Contains("late") || key.Contains("RA-"))
            {
                emblemShape = (int)((hash % 2U == 0) ? 1 : 3);
            }
            else if (lowerTitle.Contains("checkmate") || lowerTitle.Contains("sky") || lowerTitle.Contains("queen") || lowerTitle.Contains("crown"))
            {
                emblemShape = 2;
            }

            bool isAlice = key.IndexOf("-AL-", StringComparison.OrdinalIgnoreCase) >= 0 || (ownerSeat == 0 && key.IndexOf("-RA-", StringComparison.OrdinalIgnoreCase) < 0);

            // Distinct palette per card based on archetype + hash variation
            float hueShift = ((hash >> 8) & 0xFF) / 255f * 0.16f - 0.08f;
            Color bgTop;
            Color bgBottom;
            Color emblemPrimary;
            Color emblemAccent;

            if (isAlice)
            {
                bgTop = ShiftColor(new Color(0.12f, 0.26f, 0.48f, 1f), hueShift);
                bgBottom = ShiftColor(new Color(0.06f, 0.14f, 0.28f, 1f), -hueShift);
                emblemPrimary = ShiftColor(new Color(0.42f, 0.88f, 0.98f, 1f), hueShift * 0.5f);
                emblemAccent = new Color(0.98f, 0.84f, 0.36f, 1f);
            }
            else
            {
                bgTop = ShiftColor(new Color(0.44f, 0.14f, 0.22f, 1f), hueShift);
                bgBottom = ShiftColor(new Color(0.22f, 0.08f, 0.16f, 1f), -hueShift);
                emblemPrimary = ShiftColor(new Color(0.98f, 0.68f, 0.28f, 1f), hueShift * 0.5f);
                emblemAccent = new Color(0.96f, 0.34f, 0.42f, 1f);
            }

            if (string.Equals(cardType, "Counter", StringComparison.OrdinalIgnoreCase))
            {
                bgTop = Color.Lerp(bgTop, new Color(0.38f, 0.16f, 0.48f, 1f), 0.6f);
                emblemPrimary = new Color(0.88f, 0.52f, 1.0f, 1f);
            }

            Color32[] pixels = new Color32[size * size];
            float rayCount = 4f + (hash % 5U) * 2f;

            for (int y = 0; y < size; y++)
            {
                float v = (y + 0.5f) / size;
                float ny = v * 2f - 1f;
                for (int x = 0; x < size; x++)
                {
                    float u = (x + 0.5f) / size;
                    float nx = u * 2f - 1f;
                    float r = Mathf.Sqrt(nx * nx + ny * ny);
                    float angle = Mathf.Atan2(ny, nx);

                    Color col = Color.Lerp(bgBottom, bgTop, v);

                    // Subtle sunburst rays
                    float rays = 0.5f + 0.5f * Mathf.Cos(angle * rayCount + (hash & 7));
                    col = Color.Lerp(col, emblemAccent, rays * 0.12f * Mathf.Clamp01(1.1f - r));

                    // Evaluate emblem SDF
                    bool inEmblem = EvaluateEmblemShape(emblemShape, nx, ny, r, angle);
                    bool inRing = Mathf.Abs(r - 0.72f) < 0.035f;

                    if (inEmblem)
                    {
                        col = Color.Lerp(emblemPrimary, emblemAccent, Mathf.Clamp01(v * 0.7f));
                    }
                    else if (inRing)
                    {
                        col = Color.Lerp(col, emblemAccent, 0.75f);
                    }

                    // Vignette border
                    if (Mathf.Abs(nx) > 0.90f || Mathf.Abs(ny) > 0.90f)
                    {
                        col *= 0.55f;
                        col.a = 1f;
                    }

                    pixels[y * size + x] = col;
                }
            }

            tex.SetPixels32(pixels);
            tex.Apply(false, false);
            return tex;
        }

        private static bool EvaluateEmblemShape(int shape, float nx, float ny, float r, float angle)
        {
            switch (shape)
            {
                case 0: // Keyhole & Ornate Door Arch
                {
                    bool head = Vector2.Distance(new Vector2(nx, ny), new Vector2(0f, 0.18f)) < 0.24f;
                    bool body = Mathf.Abs(nx) < (0.18f - ny * 0.15f) && ny <= 0.18f && ny >= -0.48f;
                    bool innerHole = Vector2.Distance(new Vector2(nx, ny), new Vector2(0f, 0.18f)) < 0.09f;
                    return (head || body) && !innerHole;
                }
                case 1: // White Rabbit Pocket Watch & Hands
                {
                    bool rim = Mathf.Abs(r - 0.48f) < 0.06f;
                    bool crown = Mathf.Abs(nx) < 0.12f && ny > 0.46f && ny < 0.64f;
                    bool hand1 = Mathf.Abs(nx - ny * 0.5f) < 0.045f && ny >= 0f && r < 0.42f;
                    bool hand2 = Mathf.Abs(ny + nx * 0.3f) < 0.045f && nx >= 0f && r < 0.34f;
                    return rim || crown || hand1 || hand2;
                }
                case 2: // Checkmate Crown / Chess Spire
                {
                    bool baseBar = Mathf.Abs(nx) < 0.46f && ny >= -0.42f && ny <= -0.22f;
                    float spireWave = -0.22f + 0.48f * Mathf.Abs(Mathf.Cos(nx * 3.5f));
                    bool spires = Mathf.Abs(nx) < 0.46f && ny > -0.22f && ny < spireWave;
                    return baseBar || spires;
                }
                case 3: // Wonderland Suit Diamond / Heart
                {
                    float manhattan = Mathf.Abs(nx) * 1.25f + Mathf.Abs(ny) * 1.05f;
                    return manhattan < 0.54f && manhattan > 0.14f;
                }
                case 4: // Cheshire Crescent / Arcane Star
                {
                    float d1 = Vector2.Distance(new Vector2(nx, ny), new Vector2(0f, -0.05f));
                    float d2 = Vector2.Distance(new Vector2(nx, ny), new Vector2(0.14f, 0.12f));
                    return d1 < 0.48f && d2 > 0.38f;
                }
                default: // Tower / Hourglass
                {
                    bool topBottom = Mathf.Abs(nx) < 0.42f && Mathf.Abs(Mathf.Abs(ny) - 0.44f) < 0.06f;
                    bool glass = Mathf.Abs(nx) < (Mathf.Abs(ny) * 0.85f + 0.07f) && Mathf.Abs(ny) < 0.44f;
                    return topBottom || glass;
                }
            }
        }

        private static Color ShiftColor(Color c, float delta)
        {
            return new Color(
                Mathf.Clamp01(c.r + delta),
                Mathf.Clamp01(c.g + delta * 0.7f),
                Mathf.Clamp01(c.b - delta),
                1f);
        }

        private static uint ComputeDeterministicHash(string input)
        {
            uint hash = 2166136261U;
            if (string.IsNullOrEmpty(input))
            {
                return hash;
            }
            for (int i = 0; i < input.Length; i++)
            {
                hash ^= input[i];
                hash *= 16777619U;
            }
            return hash;
        }
    }
}

