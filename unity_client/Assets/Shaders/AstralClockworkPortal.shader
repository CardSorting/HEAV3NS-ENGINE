Shader "Heav3ns/AstralClockworkPortal"
{
    Properties { _Color ("Gold", Color) = (0.8,0.57,0.22,1) _MainTex ("Surface", 2D) = "white" {} }
    SubShader
    {
        Tags { "RenderType"="Opaque" }
        Pass
        {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "UnityCG.cginc"
            sampler2D _MainTex; float4 _MainTex_ST; fixed4 _Color;
            struct a { float4 vertex:POSITION; float2 uv:TEXCOORD0; };
            struct v { float4 pos:SV_POSITION; float2 uv:TEXCOORD0; };
            v vert(a i) { v o; o.pos=UnityObjectToClipPos(i.vertex); o.uv=TRANSFORM_TEX(i.uv,_MainTex); return o; }
            fixed4 frag(v i):SV_Target
            {
                float glint = pow(saturate(1.0 - abs(frac(i.uv.x + i.uv.y - _Time.y * .12) - .5) * 12.0), 4.0);
                fixed4 c = tex2D(_MainTex, i.uv) * _Color; c.rgb += glint * .45; return c;
            }
            ENDCG
        }
    }
}
