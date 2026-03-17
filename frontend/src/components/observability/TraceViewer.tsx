import React, { useState, useEffect, useRef } from 'react';
import { useWebSocket } from '../../hooks/useWebSocket';
import './TraceViewer.css';

interface AgentSpan {
  type: 'thought' | 'action' | 'observation' | 'reflection' | 'decision' | 'loop_detected' | 'error';
  timestamp: number;
  content?: string;
  tool?: string;
  params?: any;
  result?: string;
  duration_ms?: number;
  success?: boolean;
  critique?: string;
  confidence?: number;
  choices?: string[];
  selected?: string;
  reasoning?: string;
  error?: string;
}

interface TraceSession {
  execution_id: string;
  workflow_id: string;
  started_at: number;
  status: 'running' | 'paused' | 'completed' | 'failed' | 'cancelled';
}

interface TraceViewerProps {
  executionId?: string;
  autoScroll?: boolean;
}

export const TraceViewer: React.FC<TraceViewerProps> = ({ 
  executionId, 
  autoScroll = true 
}) => {
  const [spans, setSpans] = useState<AgentSpan[]>([]);
  const [session, setSession] = useState<TraceSession | null>(null);
  const [selectedSpan, setSelectedSpan] = useState<number | null>(null);
  const [isPaused, setIsPaused] = useState(false);
  const [filter, setFilter] = useState<string>('all');
  const scrollRef = useRef<HTMLDivElement>(null);
  
  const { lastMessage, sendMessage, connectionStatus } = useWebSocket(
    `ws://localhost:8080/ws/observability${executionId ? `?execution_id=${executionId}` : ''}`
  );

  useEffect(() => {
    if (lastMessage) {
      const event = JSON.parse(lastMessage.data);
      
      switch (event.event_type) {
        case 'session_started':
          setSession({
            execution_id: event.execution_id,
            workflow_id: event.workflow_id,
            started_at: event.timestamp,
            status: 'running'
          });
          setSpans([]);
          break;
          
        case 'span_recorded':
          setSpans(prev => [...prev, event.span]);
          break;
          
        case 'execution_paused':
          setIsPaused(true);
          setSession(prev => prev ? { ...prev, status: 'paused' } : null);
          break;
          
        case 'execution_resumed':
          setIsPaused(false);
          setSession(prev => prev ? { ...prev, status: 'running' } : null);
          break;
          
        case 'session_completed':
          setSession(prev => prev ? { 
            ...prev, 
            status: event.success ? 'completed' : 'failed' 
          } : null);
          break;
      }
    }
  }, [lastMessage]);

  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [spans, autoScroll]);

  const filteredSpans = spans.filter(span => {
    if (filter === 'all') return true;
    return span.type === filter;
  });

  const handleResume = (action: string, data?: any) => {
    sendMessage(JSON.stringify({
      type: 'resume_execution',
      execution_id: executionId,
      action,
      data
    }));
  };

  const renderSpan = (span: AgentSpan, index: number) => {
    const isSelected = selectedSpan === index;
    
    switch (span.type) {
      case 'thought':
        return (
          <div 
            key={index} 
            className={`trace-span thought ${isSelected ? 'selected' : ''}`}
            onClick={() => setSelectedSpan(index)}
          >
            <div className="span-header">
              <span className="span-icon">💭</span>
              <span className="span-type">Thought</span>
              <span className="span-time">{formatTime(span.timestamp)}</span>
            </div>
            <div className="span-content">{span.content}</div>
          </div>
        );
        
      case 'action':
        return (
          <div 
            key={index} 
            className={`trace-span action ${isSelected ? 'selected' : ''}`}
            onClick={() => setSelectedSpan(index)}
          >
            <div className="span-header">
              <span className="span-icon">⚡</span>
              <span className="span-type">Action</span>
              <span className="span-time">{formatTime(span.timestamp)}</span>
            </div>
            <div className="span-content">
              <div className="tool-name">{span.tool}</div>
              <pre className="tool-params">{JSON.stringify(span.params, null, 2)}</pre>
              {span.reasoning && (
                <div className="action-reasoning">{span.reasoning}</div>
              )}
            </div>
          </div>
        );
        
      case 'observation':
        return (
          <div 
            key={index} 
            className={`trace-span observation ${isSelected ? 'selected' : ''} ${span.success ? '' : 'error'}`}
            onClick={() => setSelectedSpan(index)}
          >
            <div className="span-header">
              <span className="span-icon">👁️</span>
              <span className="span-type">Observation</span>
              <span className="span-time">{formatTime(span.timestamp)}</span>
              {span.duration_ms && (
                <span className="span-duration">{span.duration_ms}ms</span>
              )}
            </div>
            <div className="span-content">
              <pre>{span.result}</pre>
            </div>
          </div>
        );
        
      case 'reflection':
        return (
          <div 
            key={index} 
            className={`trace-span reflection ${isSelected ? 'selected' : ''}`}
            onClick={() => setSelectedSpan(index)}
          >
            <div className="span-header">
              <span className="span-icon">🔄</span>
              <span className="span-type">Reflection</span>
              <span className="span-time">{formatTime(span.timestamp)}</span>
              {span.confidence && (
                <span className="confidence">{(span.confidence * 100).toFixed(0)}%</span>
              )}
            </div>
            <div className="span-content">
              <div className="critique">{span.critique}</div>
            </div>
          </div>
        );
        
      case 'decision':
        return (
          <div 
            key={index} 
            className={`trace-span decision ${isSelected ? 'selected' : ''}`}
            onClick={() => setSelectedSpan(index)}
          >
            <div className="span-header">
              <span className="span-icon">🎯</span>
              <span className="span-type">Decision</span>
              <span className="span-time">{formatTime(span.timestamp)}</span>
            </div>
            <div className="span-content">
              <div className="choices">
                {span.choices?.map((choice, i) => (
                  <div 
                    key={i} 
                    className={`choice ${choice === span.selected ? 'selected' : ''}`}
                  >
                    {choice === span.selected ? '✓ ' : ''}{choice}
                  </div>
                ))}
              </div>
              <div className="reasoning">{span.reasoning}</div>
            </div>
          </div>
        );
        
      case 'loop_detected':
        return (
          <div 
            key={index} 
            className={`trace-span loop ${isSelected ? 'selected' : ''}`}
            onClick={() => setSelectedSpan(index)}
          >
            <div className="span-header">
              <span className="span-icon">⚠️</span>
              <span className="span-type">Loop Detected</span>
              <span className="span-time">{formatTime(span.timestamp)}</span>
            </div>
            <div className="span-content">{span.content}</div>
          </div>
        );
        
      case 'error':
        return (
          <div 
            key={index} 
            className={`trace-span error ${isSelected ? 'selected' : ''}`}
            onClick={() => setSelectedSpan(index)}
          >
            <div className="span-header">
              <span className="span-icon">❌</span>
              <span className="span-type">Error</span>
              <span className="span-time">{formatTime(span.timestamp)}</span>
            </div>
            <div className="span-content">{span.error}</div>
          </div>
        );
        
      default:
        return null;
    }
  };

  return (
    <div className="trace-viewer">
      <div className="trace-header">
        <h3>Execution Trace</h3>
        {session && (
          <div className="session-info">
            <span className={`status ${session.status}`}>{session.status}</span>
            <span className="execution-id">{session.execution_id}</span>
          </div>
        )}
        <div className="trace-controls">
          <select value={filter} onChange={(e) => setFilter(e.target.value)}>
            <option value="all">All</option>
            <option value="thought">Thoughts</option>
            <option value="action">Actions</option>
            <option value="observation">Observations</option>
            <option value="reflection">Reflections</option>
            <option value="error">Errors</option>
          </select>
          <span className={`connection-status ${connectionStatus}`}>
            {connectionStatus}
          </span>
        </div>
      </div>

      {isPaused && (
        <div className="pause-banner">
          <span>⏸️ Execution Paused</span>
          <div className="pause-actions">
            <button onClick={() => handleResume('continue')}>Continue</button>
            <button onClick={() => handleResume('skip')}>Skip</button>
            <button onClick={() => handleResume('abort')}>Abort</button>
          </div>
        </div>
      )}

      <div className="trace-timeline" ref={scrollRef}>
        {filteredSpans.map((span, index) => renderSpan(span, index))}
        {filteredSpans.length === 0 && (
          <div className="empty-state">
            {session ? 'Waiting for execution...' : 'No active session'}
          </div>
        )}
      </div>

      {selectedSpan !== null && spans[selectedSpan] && (
        <div className="span-detail">
          <h4>Span Details</h4>
          <pre>{JSON.stringify(spans[selectedSpan], null, 2)}</pre>
        </div>
      )}
    </div>
  );
};

function formatTime(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString();
}

export default TraceViewer;
