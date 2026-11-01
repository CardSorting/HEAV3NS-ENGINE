using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net;
using System.Text;
using System.Threading;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

namespace Heav3ns.UnityMCP
{
    /// <summary>
    /// Unity MCP Editor Bridge listening on http://127.0.0.1:2718.
    /// Allows AI coding agents and MCP clients to inspect scene hierarchy, create/modify GameObjects,
    /// query console logs, and trigger compilation/tests in a live Unity Editor instance without cold restarts.
    /// </summary>
    [InitializeOnLoad]
    public static class UnityMcpEditorBridge
    {
        public const string DefaultHost = "127.0.0.1";
        public const int DefaultPort = 2718;

        private static HttpListener _listener;
        private static Thread _listenerThread;
        private static volatile bool _isRunning;
        private static readonly ConcurrentQueue<PendingRpcWork> _mainThreadQueue = new ConcurrentQueue<PendingRpcWork>();
        private static readonly List<string> _recentConsoleLogs = new List<string>();
        private static readonly object _logLock = new object();

        private class PendingRpcWork
        {
            public string Method;
            public string RawBody;
            public string ResultJson;
            public ManualResetEventSlim DoneEvent = new ManualResetEventSlim(false);
        }

        static UnityMcpEditorBridge()
        {
            Application.logMessageReceivedThreaded += OnLogMessageReceived;

            // Do not bind persistent background listener inside short-lived -quit batchmode runs unless explicitly requested
            if (Application.isBatchMode && Environment.GetCommandLineArgs().Contains("-quit"))
            {
                return;
            }

            EditorApplication.update += ProcessMainThreadQueue;
            AssemblyReloadEvents.beforeAssemblyReload += StopServer;
            EditorApplication.quitting += StopServer;

            StartServer();
        }

        private static void OnLogMessageReceived(string condition, string stackTrace, LogType type)
        {
            lock (_logLock)
            {
                if (_recentConsoleLogs.Count >= 100)
                {
                    _recentConsoleLogs.RemoveAt(0);
                }
                _recentConsoleLogs.Add($"[{type}] {condition}");
            }
        }

        [MenuItem("HEAV3NS/MCP/Start Localhost Bridge (127.0.0.1:2718)")]
        public static void StartServer()
        {
            if (_isRunning)
            {
                return;
            }

            try
            {
                _listener = new HttpListener();
                _listener.Prefixes.Add($"http://{DefaultHost}:{DefaultPort}/");
                _listener.Start();
                _isRunning = true;

                _listenerThread = new Thread(ListenLoop)
                {
                    IsBackground = true,
                    Name = "UnityMcpEditorBridgeListener"
                };
                _listenerThread.Start();
                Debug.Log($"[UnityMCP] Localhost MCP Editor Bridge listening on http://{DefaultHost}:{DefaultPort}/");
            }
            catch (Exception ex)
            {
                Debug.LogWarning($"[UnityMCP] Could not bind http://{DefaultHost}:{DefaultPort}/: {ex.Message}");
                _isRunning = false;
            }
        }

        [MenuItem("HEAV3NS/MCP/Stop Localhost Bridge")]
        public static void StopServer()
        {
            _isRunning = false;
            try
            {
                _listener?.Stop();
                _listener?.Close();
            }
            catch (Exception)
            {
                // Ignore listener close exceptions during domain reload
            }
            _listener = null;
        }

        private static void ListenLoop()
        {
            while (_isRunning && _listener != null && _listener.IsListening)
            {
                try
                {
                    HttpListenerContext context = _listener.GetContext();
                    ThreadPool.QueueUserWorkItem(_ => HandleHttpRequest(context));
                }
                catch (HttpListenerException)
                {
                    break;
                }
                catch (ObjectDisposedException)
                {
                    break;
                }
                catch (Exception)
                {
                    if (!_isRunning) break;
                }
            }
        }

        private static void HandleHttpRequest(HttpListenerContext ctx)
        {
            try
            {
                string path = ctx.Request.Url?.AbsolutePath ?? "/";
                string body = string.Empty;
                if (ctx.Request.HasEntityBody)
                {
                    using (var reader = new StreamReader(ctx.Request.InputStream, ctx.Request.ContentEncoding ?? Encoding.UTF8))
                    {
                        body = reader.ReadToEnd();
                    }
                }

                string method = path.Trim('/').ToLowerInvariant();
                if (string.IsNullOrEmpty(method))
                {
                    method = "ping";
                }

                var work = new PendingRpcWork
                {
                    Method = method,
                    RawBody = body
                };
                _mainThreadQueue.Enqueue(work);

                // Wait up to 5 seconds for main-thread EditorApplication.update to execute
                if (!work.DoneEvent.Wait(5000))
                {
                    WriteJsonResponse(ctx.Response, 504, "{\"status\":\"timeout\",\"bridge\":\"127.0.0.1:2718\"}");
                    return;
                }

                WriteJsonResponse(ctx.Response, 200, work.ResultJson ?? "{\"status\":\"ok\"}");
            }
            catch (Exception ex)
            {
                try
                {
                    WriteJsonResponse(ctx.Response, 500, $"{{\"status\":\"error\",\"message\":\"{EscapeJson(ex.Message)}\"}}");
                }
                catch
                {
                    // Ignore write error
                }
            }
        }

        private static void ProcessMainThreadQueue()
        {
            while (_mainThreadQueue.TryDequeue(out PendingRpcWork work))
            {
                try
                {
                    work.ResultJson = ExecuteOnMainThread(work.Method, work.RawBody);
                }
                catch (Exception ex)
                {
                    work.ResultJson = $"{{\"status\":\"error\",\"message\":\"{EscapeJson(ex.Message)}\"}}";
                }
                finally
                {
                    work.DoneEvent.Set();
                }
            }
        }

        public static string ExecuteOnMainThread(string method, string rawBody)
        {
            Scene activeScene = SceneManager.GetActiveScene();
            if (!activeScene.IsValid() || string.IsNullOrEmpty(activeScene.name))
            {
                if (File.Exists(Path.Combine(Directory.GetCurrentDirectory(), "Assets/Scenes/Battlefield.unity")))
                {
                    activeScene = EditorSceneManager.OpenScene("Assets/Scenes/Battlefield.unity", OpenSceneMode.Single);
                }
            }

            // If the client sends a JSON-RPC 2.0 payload (e.g. MCP initialize / tools/list / tools/call), handle it directly
            if (!string.IsNullOrWhiteSpace(rawBody) && rawBody.Contains("\"jsonrpc\""))
            {
                return HandleMcpJsonRpc(activeScene, rawBody);
            }

            switch (method)
            {
                case "ping":
                case "status":
                case "hierarchy":
                case "list_gameobjects":
                case "mcp":
                    return BuildHierarchySnapshotJson(activeScene);

                case "logs":
                case "console":
                    lock (_logLock)
                    {
                        string logsArray = string.Join(",", _recentConsoleLogs.Select(l => $"\"{EscapeJson(l)}\""));
                        return $"{{\"status\":\"ok\",\"count\":{_recentConsoleLogs.Count},\"logs\":[{logsArray}]}}";
                    }

                case "create_gameobject":
                    return CreateGameObjectFromBody(activeScene, rawBody);

                case "compile":
                    AssetDatabase.Refresh(ImportAssetOptions.ForceSynchronousImport);
                    return $"{{\"status\":\"ok\",\"compilationFailed\":{(EditorUtility.scriptCompilationFailed ? "true" : "false")}}}";

                default:
                    return BuildHierarchySnapshotJson(activeScene);
            }
        }

        private static string HandleMcpJsonRpc(Scene activeScene, string rawBody)
        {
            string idPart = ExtractJsonRpcId(rawBody);
            if (rawBody.Contains("\"initialize\""))
            {
                return $"{{\"jsonrpc\":\"2.0\",\"id\":{idPart},\"result\":{{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{{\"tools\":{{}}}},\"serverInfo\":{{\"name\":\"unity-mcp-bridge\",\"version\":\"1.0.0\"}}}}}}";
            }
            if (rawBody.Contains("\"notifications/initialized\""))
            {
                return "{\"jsonrpc\":\"2.0\",\"result\":{}}";
            }
            if (rawBody.Contains("\"tools/list\""))
            {
                return $"{{\"jsonrpc\":\"2.0\",\"id\":{idPart},\"result\":{{\"tools\":[" +
                       "{\"name\":\"list_gameobjects\",\"description\":\"List all GameObjects and components in the active Unity scene\",\"inputSchema\":{\"type\":\"object\",\"properties\":{}}}," +
                       "{\"name\":\"get_console_logs\",\"description\":\"Retrieve recent Unity Editor console logs\",\"inputSchema\":{\"type\":\"object\",\"properties\":{}}}," +
                       "{\"name\":\"create_gameobject\",\"description\":\"Create a new GameObject in the active scene\",\"inputSchema\":{\"type\":\"object\",\"properties\":{\"name\":{\"type\":\"string\"}}}}]" +
                       "}}";
            }
            if (rawBody.Contains("\"tools/call\""))
            {
                string payload;
                if (rawBody.Contains("\"get_console_logs\""))
                {
                    lock (_logLock)
                    {
                        string logsArray = string.Join(",", _recentConsoleLogs.Select(l => $"\"{EscapeJson(l)}\""));
                        payload = $"{{\"status\":\"ok\",\"count\":{_recentConsoleLogs.Count},\"logs\":[{logsArray}]}}";
                    }
                }
                else if (rawBody.Contains("\"create_gameobject\""))
                {
                    payload = CreateGameObjectFromBody(activeScene, rawBody);
                }
                else
                {
                    payload = BuildHierarchySnapshotJson(activeScene);
                }
                return $"{{\"jsonrpc\":\"2.0\",\"id\":{idPart},\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{EscapeJson(payload)}\"}}]}}}}";
            }

            string snapshot = BuildHierarchySnapshotJson(activeScene);
            return $"{{\"jsonrpc\":\"2.0\",\"id\":{idPart},\"result\":{snapshot}}}";
        }

        private static string CreateGameObjectFromBody(Scene activeScene, string rawBody)
        {
            string goName = "NewGameObject";
            int nameIdx = rawBody.IndexOf("\"name\"", StringComparison.OrdinalIgnoreCase);
            if (nameIdx >= 0)
            {
                int colon = rawBody.IndexOf(':', nameIdx);
                int q1 = colon >= 0 ? rawBody.IndexOf('"', colon + 1) : -1;
                int q2 = q1 >= 0 ? rawBody.IndexOf('"', q1 + 1) : -1;
                if (q1 >= 0 && q2 > q1)
                {
                    string parsedName = rawBody.Substring(q1 + 1, q2 - q1 - 1);
                    if (!string.IsNullOrWhiteSpace(parsedName) && parsedName != "create_gameobject")
                    {
                        goName = parsedName;
                    }
                }
            }
            GameObject created = new GameObject(goName);
            SceneManager.MoveGameObjectToScene(created, activeScene);
            EditorSceneManager.MarkSceneDirty(activeScene);
            return $"{{\"status\":\"ok\",\"created\":\"{EscapeJson(created.name)}\",\"path\":\"{EscapeJson(GetGameObjectPath(created.transform))}\"}}";
        }

        private static string ExtractJsonRpcId(string rawBody)
        {
            int idx = rawBody.IndexOf("\"id\"", StringComparison.Ordinal);
            if (idx < 0) return "1";
            int colon = rawBody.IndexOf(':', idx);
            if (colon < 0) return "1";
            int start = colon + 1;
            while (start < rawBody.Length && char.IsWhiteSpace(rawBody[start])) start++;
            if (start >= rawBody.Length) return "1";
            if (rawBody[start] == '"')
            {
                int endQuote = rawBody.IndexOf('"', start + 1);
                if (endQuote > start) return rawBody.Substring(start, endQuote - start + 1);
            }
            int end = start;
            while (end < rawBody.Length && (char.IsDigit(rawBody[end]) || rawBody[end] == '-')) end++;
            return end > start ? rawBody.Substring(start, end - start) : "1";
        }

        public static string BuildHierarchySnapshotJson(Scene scene)
        {
            var sb = new StringBuilder();
            sb.Append("{");
            sb.Append("\"status\":\"ok\",");
            sb.Append($"\"bridge\":\"http://{DefaultHost}:{DefaultPort}\",");
            sb.Append($"\"unityVersion\":\"{EscapeJson(Application.unityVersion)}\",");
            sb.Append($"\"runInBackground\":{(Application.runInBackground ? "true" : "false")},");
            sb.Append($"\"sceneName\":\"{EscapeJson(scene.name)}\",");
            sb.Append($"\"scenePath\":\"{EscapeJson(scene.path)}\",");
            sb.Append("\"gameObjects\":[");

            GameObject[] roots = scene.IsValid() ? scene.GetRootGameObjects() : Array.Empty<GameObject>();
            var allObjects = new List<GameObject>();
            foreach (GameObject root in roots)
            {
                CollectHierarchy(root, allObjects);
            }

            for (int i = 0; i < allObjects.Count; i++)
            {
                GameObject go = allObjects[i];
                if (i > 0) sb.Append(",");
                string[] components = go.GetComponents<Component>()
                    .Where(c => c != null)
                    .Select(c => $"\"{EscapeJson(c.GetType().Name)}\"")
                    .ToArray();
                Vector3 pos = go.transform.position;
                Vector3 rot = go.transform.eulerAngles;
                sb.Append("{");
                sb.Append($"\"name\":\"{EscapeJson(go.name)}\",");
                sb.Append($"\"path\":\"{EscapeJson(GetGameObjectPath(go.transform))}\",");
                sb.Append($"\"activeSelf\":{(go.activeSelf ? "true" : "false")},");
                sb.Append($"\"tag\":\"{EscapeJson(go.tag)}\",");
                sb.AppendFormat(
                    System.Globalization.CultureInfo.InvariantCulture,
                    "\"position\":[{0:F2},{1:F2},{2:F2}],\"rotation\":[{3:F2},{4:F2},{5:F2}],",
                    pos.x, pos.y, pos.z, rot.x, rot.y, rot.z);
                sb.Append($"\"components\":[{string.Join(",", components)}]");
                sb.Append("}");
            }

            sb.Append("]}");
            return sb.ToString();
        }

        private static void CollectHierarchy(GameObject current, List<GameObject> output)
        {
            output.Add(current);
            for (int i = 0; i < current.transform.childCount; i++)
            {
                CollectHierarchy(current.transform.GetChild(i).gameObject, output);
            }
        }

        private static string GetGameObjectPath(Transform t)
        {
            if (t.parent == null) return t.name;
            return GetGameObjectPath(t.parent) + "/" + t.name;
        }

        private static void WriteJsonResponse(HttpListenerResponse response, int statusCode, string json)
        {
            byte[] bytes = Encoding.UTF8.GetBytes(json);
            response.StatusCode = statusCode;
            response.ContentType = "application/json; charset=utf-8";
            response.ContentLength64 = bytes.Length;
            using (Stream output = response.OutputStream)
            {
                output.Write(bytes, 0, bytes.Length);
            }
        }

        private static string EscapeJson(string s)
        {
            if (string.IsNullOrEmpty(s)) return string.Empty;
            return s.Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\n", "\\n").Replace("\r", "\\r");
        }
    }
}

