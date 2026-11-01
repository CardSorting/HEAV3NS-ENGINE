Shader "Heav3ns/AnimeStandeeBreathing"
{
    Properties
    {
        _MainTex ("Portrait", 2D) = "white" {}
        _Color ("Tint", Color) = (1,1,1,1)
        _Breath ("Breath", Range(0,0.06)) = 0.008
        _Gaze ("Cursor parallax", Vector) = (0,0,0,0)
        _EyeRegion ("Eye region XY and half-size ZW", Vector) = (0.55,0.87,0.12,0.025)
    }
    SubShader
    {
        Tags { "Queue"="Transparent" "RenderType"="Transparent" }
        Cull Off ZWrite Off Blend SrcAlpha OneMinusSrcAlpha
        Pass
        {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "UnityCG.cginc"
            sampler2D _MainTex; float4 _MainTex_ST; fixed4 _Color; float _Breath; float4 _Gaze; float4 _EyeRegion;
            struct appdata { float4 vertex : POSITION; float2 uv : TEXCOORD0; };
            struct v2f { float4 position : SV_POSITION; float2 uv : TEXCOORD0; };
            v2f vert(appdata v)
            {
                v2f o; float weight = saturate(v.uv.y);
                v.vertex.x += sin(_Time.y * 1.7 + v.uv.y * 3.0) * _Breath * weight;
                v.vertex.x += _Gaze.x * weight * 0.012;
                v.vertex.y += sin(_Time.y * 1.7) * _Breath * weight;
                o.position = UnityObjectToClipPos(v.vertex); o.uv = TRANSFORM_TEX(v.uv, _MainTex); return o;
            }
            fixed4 frag(v2f i) : SV_Target
            {
                float2 uv = i.uv;
                float2 d = abs((uv - _EyeRegion.xy) / max(_EyeRegion.zw, float2(.001, .001)));
                float mask = (1.0 - smoothstep(.72, 1.0, d.x)) * (1.0 - smoothstep(.55, 1.0, d.y));
                float phase = frac(_Time.y / 4.7);
                float blink = smoothstep(.94, .97, phase) * (1.0 - smoothstep(.97, 1.0, phase));
                uv.y = lerp(uv.y, _EyeRegion.y + (uv.y - _EyeRegion.y) * .15, mask * blink);
                return tex2D(_MainTex, uv) * _Color;
            }
            ENDCG
        }
    }
}
