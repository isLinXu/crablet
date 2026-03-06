import sys
import json

def main():
    try:
        # Read from stdin line by line
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue

            try:
                request = json.loads(line)
            except json.JSONDecodeError:
                continue

            msg_type = "request" if "id" in request else "notification"
            method = request.get("method")
            msg_id = request.get("id")

            # 1. Initialize Handshake
            if method == "initialize":
                response = {
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "PythonMathMCP",
                            "version": "0.1.0"
                        }
                    }
                }
                print(json.dumps(response))
                sys.stdout.flush()

            # 2. Tools List
            elif method == "tools/list":
                response = {
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "result": {
                        "tools": [
                            {
                                "name": "add_numbers",
                                "description": "Add two numbers together via MCP",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "a": {"type": "number", "description": "First number"},
                                        "b": {"type": "number", "description": "Second number"}
                                    },
                                    "required": ["a", "b"]
                                }
                            }
                        ]
                    }
                }
                print(json.dumps(response))
                sys.stdout.flush()

            # 3. Call Tool
            elif method == "tools/call":
                params = request.get("params", {})
                name = params.get("name")
                args = params.get("arguments", {})

                if name == "add_numbers":
                    try:
                        a = float(args.get("a", 0))
                        b = float(args.get("b", 0))
                        result = a + b

                        response = {
                            "jsonrpc": "2.0",
                            "id": msg_id,
                            "result": {
                                "content": [
                                    {
                                        "type": "text",
                                        "text": str(result)
                                    }
                                ],
                                "isError": False
                            }
                        }
                    except Exception as e:
                        response = {
                            "jsonrpc": "2.0",
                            "id": msg_id,
                            "result": {
                                "content": [
                                    {
                                        "type": "text",
                                        "text": f"Error: {str(e)}"
                                    }
                                ],
                                "isError": True
                            }
                        }
                else:
                    response = {
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "error": {
                            "code": -32601,
                            "message": f"Method not found: {name}"
                        }
                    }

                print(json.dumps(response))
                sys.stdout.flush()

            # Ignore notifications like 'notifications/initialized'
            elif msg_type == "notification":
                pass

    except KeyboardInterrupt:
        sys.exit(0)

if __name__ == "__main__":
    main()
