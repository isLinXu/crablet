import requests
import json
import sseclient
import threading
import time

BASE_URL = "http://localhost:18789/api/v1"

def test_dashboard_stats():
    print("Testing Dashboard Stats...")
    try:
        resp = requests.get(f"{BASE_URL}/dashboard")
        if resp.status_code == 200:
            print("✅ Dashboard Stats OK")
            print(json.dumps(resp.json(), indent=2))
        else:
            print(f"❌ Dashboard Stats Failed: {resp.status_code} - {resp.text}")
    except Exception as e:
        print(f"❌ Dashboard Stats Error: {e}")

def test_sync_chat():
    print("\nTesting Sync Chat...")
    payload = {
        "message": "Hello, who are you?",
        "session_id": "test-sync-session"
    }
    try:
        resp = requests.post(f"{BASE_URL}/chat", json=payload)
        if resp.status_code == 200:
            data = resp.json()
            print("✅ Sync Chat OK")
            print(f"Response: {data.get('response')}")
            if 'cognitive_layer' in data:
                print(f"Cognitive Layer: {data['cognitive_layer']}")
        else:
            print(f"❌ Sync Chat Failed: {resp.status_code} - {resp.text}")
    except Exception as e:
        print(f"❌ Sync Chat Error: {e}")

def test_stream_chat_rag():
    print("\nTesting Stream Chat (with RAG trigger)...")
    # Using a query likely to trigger RAG if knowledge exists, or at least pass through the pipeline
    payload = {
        "message": "Research Rust async patterns", 
        "session_id": "test-stream-session"
    }
    
    url = f"{BASE_URL}/chat/stream"
    try:
        resp = requests.post(url, json=payload, stream=True)
        client = sseclient.SSEClient(resp)
        
        received_trace = False
        received_content = False
        full_content = ""
        
        print("Stream chunks:")
        for event in client.events():
            try:
                data = json.loads(event.data)
                chunk_type = data.get("type")
                
                if chunk_type == "delta":
                    content = data.get("content", "")
                    full_content += content
                    received_content = True
                    print(".", end="", flush=True)
                elif chunk_type == "trace":
                    print("\n[TRACE] Received Trace Event")
                    payload = data.get("payload", {})
                    print(json.dumps(payload, indent=2))
                    received_trace = True
                elif chunk_type == "error":
                    print(f"\n❌ Stream Error: {data.get('content')}")
            except json.JSONDecodeError:
                pass
                
        print("\n")
        if received_content:
            print(f"✅ Stream Content Received: {full_content[:50]}...")
        else:
            print("❌ No Content Received")
            
        if received_trace:
            print("✅ RAG Trace Received")
        else:
            print("⚠️ No RAG Trace Received (Maybe no knowledge base loaded or RAG trigger failed)")

    except Exception as e:
        print(f"❌ Stream Chat Error: {e}")

if __name__ == "__main__":
    # Wait a bit for server to be fully ready if just restarted
    time.sleep(2)
    
    test_dashboard_stats()
    test_sync_chat()
    test_stream_chat_rag()
