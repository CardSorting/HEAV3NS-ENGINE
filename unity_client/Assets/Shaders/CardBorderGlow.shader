Shader "Heav3ns/CardBorderGlow"
{
    Properties
    {
        _MainTex ("Texture", 2D) = "white" {}
        _Color ("Base Color", Color) = (0.28, 0.52, 0.78, 1.0)
        _BaseColor ("Base Color", Color) = (0.28, 0.52, 0.78, 1.0)
        _GlowColor ("Glow Primary Color", Color) = (0.204, 0.827, 0.600, 1.0)
        _GlowSecondaryColor ("Glow Secondary Color", Color) = (0.984, 0.749, 0.141, 1.0)
        _GlowIntensity ("Glow Intensity", Range(0, 4)) = 0.0
        _PulseSpeed ("Pulse Speed", Range(0, 16)) = 3.2
        _HighlightState ("Highlight State (0=None,1=Playable,2=Ready,3=Target)", Float) = 0.0
        _DissolveAmount ("Dissolve Amount", Range(0, 1)) = 0.0
        _BurnEdgeColor ("Dissolve Burn Edge Color", Color) = (1.0, 0.45, 0.12, 1.0)
    }

    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" }
        LOD 200
        Cull Back

        Pass
        {
            Name "CardBorderGlowPass"
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
                UNITY_DEFINE_INSTANCED_PROP(float4, _GlowColor)
                UNITY_DEFINE_INSTANCED_PROP(float4, _GlowSecondaryColor)
                UNITY_DEFINE_INSTANCED_PROP(float, _GlowIntensity)
                UNITY_DEFINE_INSTANCED_PROP(float, _PulseSpeed)
                UNITY_DEFINE_INSTANCED_PROP(float, _HighlightState)
                UNITY_DEFINE_INSTANCED_PROP(float, _DissolveAmount)
                UNITY_DEFINE_INSTANCED_PROP(float4, _BurnEdgeColor)
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
                float3 objPos : TEXCOORD3;
                UNITY_VERTEX_INPUT_INSTANCE_ID
            };

            float hash21(float2 p)
            {
                p = frac(p * float2(123.34, 456.21));
                p += dot(p, p + 45.32);
                return frac(p.x * p.y);
            }

            float valueNoise(float2 p)
            {
                float2 i = floor(p);
                float2 f = frac(p);
                f = f * f * (3.0 - 2.0 * f);
                float a = hash21(i);
                float b = hash21(i + float2(1.0, 0.0));
                float c = hash21(i + float2(0.0, 1.0));
                float d = hash21(i + float2(1.0, 1.0));
                return lerp(lerp(a, b, f.x), lerp(c, d, f.x), f.y);
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
                o.objPos = v.vertex.xyz;
                return o;
            }

            fixed4 frag(v2f i) : SV_Target
            {
                UNITY_SETUP_INSTANCE_ID(i);

                float dissolve = UNITY_ACCESS_INSTANCED_PROP(Props, _DissolveAmount);
                float n = valueNoise(i.uv * 12.0 + i.objPos.xz * 6.0);
                float clipVal = n - dissolve;
                clip(clipVal);

                float4 baseCol = UNITY_ACCESS_INSTANCED_PROP(Props, _BaseColor);
                float4 glowCol1 = UNITY_ACCESS_INSTANCED_PROP(Props, _GlowColor);
                float4 glowCol2 = UNITY_ACCESS_INSTANCED_PROP(Props, _GlowSecondaryColor);
                float glowIntensity = UNITY_ACCESS_INSTANCED_PROP(Props, _GlowIntensity);
                float pulseSpeed = UNITY_ACCESS_INSTANCED_PROP(Props, _PulseSpeed);
                float highlightState = UNITY_ACCESS_INSTANCED_PROP(Props, _HighlightState);
                float4 burnEdge = UNITY_ACCESS_INSTANCED_PROP(Props, _BurnEdgeColor);

                // Rim & edge distance highlight
                float2 edgeDist = abs(i.uv - 0.5) * 2.0;
                float borderMask = saturate(max(edgeDist.x, edgeDist.y));
                borderMask = smoothstep(0.55, 0.98, borderMask);

                float ndotv = saturate(dot(normalize(i.worldNormal), normalize(i.viewDir)));
                float fresnel = pow(1.0 - ndotv, 2.2);

                float pulse = 0.5 + 0.5 * sin(_Time.y * pulseSpeed + (i.uv.x + i.uv.y) * 4.0);
                float4 activeGlow = lerp(glowCol1, glowCol2, pulse);

                float stateFactor = step(0.5, highlightState) * glowIntensity;
                float3 finalRgb = baseCol.rgb * tex2D(_MainTex, i.uv).rgb * (0.78 + 0.22 * ndotv);
                finalRgb += activeGlow.rgb * stateFactor * (0.45 + 0.55 * max(borderMask, fresnel));

                // Dissolve ember edge
                if (dissolve > 0.001 && clipVal < 0.09)
                {
                    float burnFactor = 1.0 - saturate(clipVal / 0.09);
                    finalRgb = lerp(finalRgb, burnEdge.rgb * 2.2, burnFactor);
                }

                return fixed4(finalRgb, 1.0);
            }
            ENDCG
        }
    }
    FallBack "Diffuse"
}

