import sys
import json
import traceback

def log(msg):
    sys.stderr.write(f"[TestMCP] {msg}\n")
    sys.stderr.flush()

def main():
    log("Starting Test MCP Server...")
    try:
        while True:
            try:
                line = sys.stdin.readline()
                if not line:
                    log("Stdin closed, exiting.")
                    break
                
                line = line.strip()
                if not line:
                    continue

                try:
                    request = json.loads(line)
                except json.JSONDecodeError:
                    log(f"JSON Decode Error: {line}")
                    continue

                msg_type = "request" if "id" in request else "notification"
                method = request.get("method")
                msg_id = request.get("id")
                
                # log(f"Received {msg_type}: {method} (ID: {msg_id})")

                response = None

                # 1. Initialize
                if method == "initialize":
                    response = {
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "result": {
                            "protocolVersion": "2024-11-05",
                            "capabilities": {
                                "tools": {},
                                "resources": {},
                                "prompts": {}
                            },
                            "serverInfo": {
                                "name": "TestMathServer",
                                "version": "0.1.0"
                            }
                        }
                    }

                # 2. Tools List
                elif method == "tools/list":
                    response = {
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "result": {
                            "tools": [
                                {
                                    "name": "mcp_add",
                                    "description": "Add two numbers via MCP",
                                    "inputSchema": {
                                        "type": "object",
                                        "properties": {
                                            "a": {"type": "number"},
                                            "b": {"type": "number"}
                                        },
                                        "required": ["a", "b"]
                                    }
                                }
                            ]
                        }
                    }

                # 3. Call Tool
                elif method == "tools/call":
                    params = request.get("params", {})
                    name = params.get("name")
                    args = params.get("arguments", {})

                    if name == "mcp_add":
                        try:
                            a = float(args.get("a", 0))
                            b = float(args.get("b", 0))
                            result = a + b
                            response = {
                                "jsonrpc": "2.0",
                                "id": msg_id,
                                "result": {
                                    "content": [{"type": "text", "text": str(result)}],
                                    "isError": False
                                }
                            }
                        except Exception as e:
                            response = {
                                "jsonrpc": "2.0",
                                "id": msg_id,
                                "result": {
                                    "content": [{"type": "text", "text": f"Error: {str(e)}"}],
                                    "isError": True
                                }
                            }
                    else:
                        response = {
                            "jsonrpc": "2.0",
                            "id": msg_id,
                            "error": {"code": -32601, "message": f"Method not found: {name}"}
                        }

                # 4. Resources List
                elif method == "resources/list":
                    response = {
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "result": {
                            "resources": []
                        }
                    }

                # 5. Prompts List
                elif method == "prompts/list":
                    response = {
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "result": {
                            "prompts": []
                        }
                    }
                
                # 6. Notifications
                elif msg_type == "notification":
                    # Ignore notifications
                    continue

                # 7. Unknown Method (Request)
                else:
                    if msg_type == "request":
                        log(f"Unknown method: {method}")
                        response = {
                            "jsonrpc": "2.0",
                            "id": msg_id,
                            "error": {"code": -32601, "message": f"Method not found: {method}"}
                        }

                # Send Response
                if response:
                    print(json.dumps(response))
                    sys.stdout.flush()

            except Exception as e:
                log(f"Error processing line: {e}")
                traceback.print_exc(file=sys.stderr)

    except KeyboardInterrupt:
        sys.exit(0)

if __name__ == "__main__":
    main()
