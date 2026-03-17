# Crablet HEARTBEAT - 心跳配置

> **定时任务** | **Scheduled Tasks**  
> **作用**: 自动化维护与后台任务  
> **最后更新**: 2026-03-15

---

## 心跳机制概述

Crablet 的心跳系统负责：
- **定期维护**: 数据清理、索引优化
- **自动任务**: 记忆整理、报告生成
- **健康检查**: 系统状态监控
- **外部同步**: 数据备份、状态上报

---

## 定时任务配置

### 每日任务 (Daily)

```yaml
daily_tasks:
  - name: "日志归档"
    schedule: "00:00"
    action: "archive_daily_logs"
    description: "将当日日志归档到历史存储"
    
  - name: "记忆提取"
    schedule: "02:00"
    action: "extract_memories"
    description: "从当日对话中提取长期记忆"
    
  - name: "索引优化"
    schedule: "03:00"
    action: "optimize_indices"
    description: "优化向量数据库索引"
    
  - name: "数据备份"
    schedule: "04:00"
    action: "backup_data"
    description: "备份用户数据到远程存储"
```

### 每周任务 (Weekly)

```yaml
weekly_tasks:
  - name: "记忆整理"
    schedule: "Sunday 01:00"
    action: "consolidate_memories"
    description: "合并重复记忆，清理过期内容"
    
  - name: "用户画像更新"
    schedule: "Sunday 03:00"
    action: "update_user_profile"
    description: "基于近期交互更新用户画像"
    
  - name: "性能报告"
    schedule: "Sunday 06:00"
    action: "generate_performance_report"
    description: "生成系统性能报告"
```

### 每月任务 (Monthly)

```yaml
monthly_tasks:
  - name: "深度分析"
    schedule: "1st 02:00"
    action: "deep_analysis"
    description: "用户行为深度分析"
    
  - name: "存储清理"
    schedule: "1st 04:00"
    action: "cleanup_storage"
    description: "清理临时文件和过期数据"
    
  - name: "模型评估"
    schedule: "15th 02:00"
    action: "evaluate_models"
    description: "评估模型性能和效果"
```

---

## 触发器配置

### 事件触发 (Event Triggers)

```yaml
event_triggers:
  - name: "会话开始"
    event: "session.start"
    action: "load_user_context"
    
  - name: "会话结束"
    event: "session.end"
    action: "save_session_summary"
    
  - name: "文件上传"
    event: "file.upload"
    action: "process_and_index"
    
  - name: "错误发生"
    event: "system.error"
    action: "log_and_alert"
```

### 条件触发 (Conditional Triggers)

```yaml
conditional_triggers:
  - name: "存储告警"
    condition: "storage_usage > 80%"
    action: "cleanup_old_data"
    
  - name: "性能告警"
    condition: "response_time > 5s"
    action: "optimize_performance"
    
  - name: "异常检测"
    condition: "error_rate > 1%"
    action: "investigate_issues"
```

---

## 健康检查

### 系统健康指标

```yaml
health_checks:
  - name: "数据库连接"
    interval: "1m"
    check: "db.ping()"
    alert_on_failure: true
    
  - name: "向量索引"
    interval: "5m"
    check: "vector_index.status()"
    alert_on_failure: true
    
  - name: "存储空间"
    interval: "1h"
    check: "storage.available_space > 10GB"
    alert_on_failure: true
    
  - name: "内存使用"
    interval: "5m"
    check: "memory.usage < 80%"
    alert_on_failure: false
```

### 健康状态定义

| 状态 | 描述 | 响应动作 |
|------|------|----------|
| 🟢 Healthy | 所有指标正常 | 正常运行 |
| 🟡 Warning | 部分指标接近阈值 | 记录日志，准备告警 |
| 🔴 Critical | 关键指标异常 | 立即告警，启动恢复程序 |
| ⚫ Down | 服务不可用 | 紧急告警，切换备用服务 |

---

## 自动化工作流

### 工作流 1: 每日维护

```
[00:00 触发] → [归档日志] → [提取记忆] → [优化索引] → [生成日报] → [发送通知]
```

### 工作流 2: 异常处理

```
[检测到异常] → {严重程度?}
    ├── [警告] → [记录日志]
    └── [严重] → [发送告警] → [尝试恢复] → {恢复成功?}
                                            ├── [是] → [恢复正常]
                                            └── [否] → [升级处理]
```

---

## 配置示例

### 最小配置

```yaml
heartbeat:
  enabled: true
  timezone: "Asia/Shanghai"
  
  tasks:
    daily:
      - name: "backup"
        schedule: "04:00"
        action: "backup_data"
    
    weekly:
      - name: "cleanup"
        schedule: "Sunday 02:00"
        action: "cleanup_storage"
```

### 完整配置

```yaml
heartbeat:
  enabled: true
  timezone: "Asia/Shanghai"
  log_level: "info"
  
  notification:
    channels:
      - type: "email"
        recipients: ["admin@example.com"]
      - type: "webhook"
        url: "https://hooks.example.com/alerts"
  
  tasks:
    daily:
      - name: "archive_logs"
        schedule: "00:00"
        action: "archive_daily_logs"
        retry: 3
        
      - name: "extract_memories"
        schedule: "02:00"
        action: "extract_memories"
        retry: 3
        
    weekly:
      - name: "consolidate"
        schedule: "Sunday 01:00"
        action: "consolidate_memories"
        retry: 2
        
    monthly:
      - name: "analysis"
        schedule: "1st 02:00"
        action: "deep_analysis"
        retry: 2
  
  health_checks:
    - name: "database"
      interval: "1m"
      timeout: "10s"
      
    - name: "storage"
      interval: "1h"
      threshold: "80%"
```

---

## 监控仪表板

### 关键指标

| 指标 | 说明 | 正常范围 |
|------|------|----------|
| 任务成功率 | 定时任务成功执行比例 | > 95% |
| 平均响应时间 | 健康检查响应时间 | < 1s |
| 存储使用率 | 数据存储使用比例 | < 80% |
| 内存使用率 | 系统内存使用比例 | < 80% |
| 错误率 | 系统错误发生频率 | < 0.1% |

---

*本文件配置 Crablet 的自动化维护任务，确保系统健康运行并持续优化。*
