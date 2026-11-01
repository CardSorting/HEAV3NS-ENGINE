Shader "Heav3ns/CardSurfaceFoil"
{
    Properties
    {
        _MainTex ("Card Artwork Texture", 2D) = "white" {}
        _Color ("Tint Color", Color) = (1, 1, 1, 1)
        _BaseColor ("Base Color", Color) = (1, 1, 1, 1)
        _FoilIntensity ("Holographic Foil Intensity", Range(0, 2)) = 0.48
        _TiltAngles ("Spring Pitch & Roll Angles (XY)", Vector) = (0, 0, 0, 0)
        _DissolveAmount ("Dissolve Clip Threshold", Range(0, 1)) = 0.0
        _DissolveEdgeWidth ("Dissolve Burn Width", Range(0.01, 0.25)) = 0.08
        _DissolveEdgeColor ("Dissolve Ember Color", Color) = (1.0, 0.48, 0.14, 1.0)
    }

    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" }
        LOD 200
        Cull Off

        Pass
        {
            Name "CardSurfaceFoilPass"
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma multi_compile_instancing
            #include "UnityCG.cginc"

            sampler2D _MainTex;
            float4 _MainTex_ST;

            UNITY_INSTANCING_BUFFER_START(Props)
                UNITY_DEFINE_INSTANCED_PROP(float4, _Color)
                UNITY_DEFINE_INSTANCED_PROP(float4, _BaseColor)
                UNITY_DEFINE_INSTANCED_PROP(float, _FoilIntensity)
                UNITY_DEFINE_INSTANCED_PROP(float4, _TiltAngles)
                UNITY_DEFINE_INSTANCED_PROP(float, _DissolveAmount)
                UNITY_DEFINE_INSTANCED_PROP(float, _DissolveEdgeWidth)
                UNITY_DEFINE_INSTANCED_PROP(float4, _DissolveEdgeColor)
            UNITY_INSTANCING_BUFFER_END(Props)

            struct appdata
            {
                float4 vertex : POSITION;
                float3 normal : NORMAL;
                float2 uv : TEXCOORD0;
                UNITY_VERTEX_INPUT_INSTANCE_ID
            };

            struct v2f
            {
                float4 pos : SV_POSITION;
                float2 uv : TEXCOORD0;
                float3 worldNormal : TEXCOORD1;
                float3 viewDir : TEXCOORD2;
                UNITY_VERTEX_INPUT_INSTANCE_ID
            };

            float hash21(float2 p)
            {
                p = frac(p * float2(234.17, 567.89));
                p += dot(p, p + 34.56);
                return frac(p.x * p.y);
            }

            float fbmNoise(float2 uv)
            {
                float2 i = floor(uv);
                float2 f = frac(uv);
                f = f * f * (3.0 - 2.0 * f);
                float a = hash21(i);
                float b = hash21(i + float2(1.0, 0.0));
                float c = hash21(i + float2(0.0, 1.0));
                float d = hash21(i + float2(1.0, 1.0));
                return lerp(lerp(a, b, f.x), lerp(c, d, f.x), f.y);
            }

            float3 rainbowSpectrum(float t)
            {
                float3 c = 0.5 + 0.5 * cos(6.28318 * (t + float3(0.0, 0.33, 0.67)));
                return c;
            }

            v2f vert(appdata v)
            {
                v2f o;
                UNITY_SETUP_INSTANCE_ID(v);
                UNITY_TRANSFER_INSTANCE_ID(v, o);
                o.pos = UnityObjectToClipPos(v.vertex);
                o.uv = TRANSFORM_TEX(v.uv, _MainTex);
                o.worldNormal = UnityObjectToWorldNormal(v.normal);
                float3 worldPos = mul(unity_ObjectToWorld, v.vertex).xyz;
                o.viewDir = normalize(_WorldSpaceCameraPos.xyz - worldPos);
                return o;
            }

            fixed4 frag(v2f i) : SV_Target
            {
                UNITY_SETUP_INSTANCE_ID(i);

                float dissolve = UNITY_ACCESS_INSTANCED_PROP(Props, _DissolveAmount);
                float edgeWidth = UNITY_ACCESS_INSTANCED_PROP(Props, _DissolveEdgeWidth);
                float4 edgeColor = UNITY_ACCESS_INSTANCED_PROP(Props, _DissolveEdgeColor);

                // Multi-octave procedural noise for burn/dissolve threshold
                float n1 = fbmNoise(i.uv * 10.0);
                float n2 = fbmNoise(i.uv * 22.0) * 0.35;
                float noiseVal = saturate(n1 * 0.75 + n2 * 0.25);

                float clipDelta = noiseVal - dissolve;
                clip(clipDelta);

                fixed4 texCol = tex2D(_MainTex, i.uv);
                float4 baseTint = UNITY_ACCESS_INSTANCED_PROP(Props, _BaseColor);
                float foilStrength = UNITY_ACCESS_INSTANCED_PROP(Props, _FoilIntensity);
                float4 tilt = UNITY_ACCESS_INSTANCED_PROP(Props, _TiltAngles);

                // View-direction & SecondOrderSpring pitch/roll dependent holographic foil sheen
                float3 V = normalize(i.viewDir);
                float3 N = normalize(i.worldNormal);
                float tiltShift = (tilt.x * 0.045) + (tilt.y * 0.045) + (V.x * 0.65) + (V.z * 0.45);
                float diagWave = sin((i.uv.x * 6.5 + i.uv.y * 4.5 + tiltShift * 4.0) * 3.14159);
                float sheenBand = smoothstep(0.15, 0.95, diagWave * 0.5 + 0.5);

                float3 holoRgb = rainbowSpectrum(i.uv.x * 1.15 - i.uv.y * 0.85 + tiltShift * 1.6);
                float3 artRgb = texCol.rgb * baseTint.rgb;

                // Blend holographic foil sheen with artwork
                float tiltMagnitude = saturate((abs(tilt.x) + abs(tilt.y)) / 18.0 + 0.35);
                float3 finalRgb = artRgb + holoRgb * sheenBand * foilStrength * tiltMagnitude * 0.55;

                // Glowing ember edge along dissolve boundary
                if (dissolve > 0.001 && clipDelta < edgeWidth)
                {
                    float burnT = 1.0 - saturate(clipDelta / max(0.001, edgeWidth));
                    finalRgb = lerp(finalRgb, edgeColor.rgb * 2.5, burnT);
                }

                return fixed4(finalRgb, texCol.a * baseTint.a);
            }
            ENDCG
        }
    }
    FallBack "Diffuse"
}

