import json
import sys

def main():
    # Read messages from stdin
    for line in sys.stdin:
        try:
            request = json.loads(line)
        except json.JSONDecodeError:
            continue

        method = request.get("method")
        msg_id = request.get("id")

        if method == "initialize":
            response = {
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "serverInfo": {"name": "TestMathServer", "version": "0.1.0"}
                }
            }
            print(json.dumps(response), flush=True)

        elif method == "notifications/initialized":
            # No response needed for notifications
            pass

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
            print(json.dumps(response), flush=True)

        elif method == "tools/call":
            params = request.get("params", {})
            name = params.get("name")
            args = params.get("arguments", {})

            if name == "mcp_add":
                a = args.get("a", 0)
                b = args.get("b", 0)
                result = a + b
                response = {
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "result": {
                        "content": [
                            {"type": "text", "text": str(result)}
                        ],
                        "isError": False
                    }
                }
                print(json.dumps(response), flush=True)
            else:
                # Unknown tool
                response = {
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "error": {"code": -32601, "message": "Method not found"}
                }
                print(json.dumps(response), flush=True)

if __name__ == "__main__":
    main()
