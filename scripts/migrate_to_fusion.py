#!/usr/bin/env python3
"""
Crablet Fusion Migration Script

This script helps migrate from the legacy Crablet memory system
to the new Fusion Memory System (OpenClaw-style four-layer architecture).

Usage:
    python migrate_to_fusion.py --workspace ./agent-workspace --source ./data

Features:
    - Migrates Core Memory to SOUL.md
    - Converts Episodic memories to Daily Logs
    - Transforms Working Memory sessions
    - Preserves all existing data
"""

import os
import sys
import json
import shutil
import argparse
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Optional


class FusionMigrator:
    """Handles migration from legacy to Fusion Memory System"""
    
    def __init__(self, source_dir: Path, workspace_dir: Path, dry_run: bool = False):
        self.source_dir = Path(source_dir)
        self.workspace_dir = Path(workspace_dir)
        self.dry_run = dry_run
        self.stats = {
            'core_memory_blocks': 0,
            'episodic_memories': 0,
            'working_sessions': 0,
            'daily_logs_created': 0,
        }
    
    def migrate(self) -> Dict:
        """Run full migration"""
        print("🦀 Crablet Fusion Migration Tool")
        print("=" * 50)
        print(f"Source: {self.source_dir}")
        print(f"Workspace: {self.workspace_dir}")
        print(f"Dry run: {self.dry_run}")
        print("=" * 50)
        
        # Create workspace structure
        self._create_workspace()
        
        # Migrate Core Memory
        self._migrate_core_memory()
        
        # Migrate Episodic memories
        self._migrate_episodic()
        
        # Migrate Working Memory sessions
        self._migrate_working_sessions()
        
        # Create initial Daily Log
        self._create_initial_daily_log()
        
        # Print summary
        self._print_summary()
        
        return self.stats
    
    def _create_workspace(self):
        """Create Fusion workspace directory structure"""
        print("\n📁 Creating workspace structure...")
        
        dirs = [
            self.workspace_dir,
            self.workspace_dir / "memory",
            self.workspace_dir / "skills",
            self.workspace_dir / "skills" / "local",
            self.workspace_dir / "skills" / "mcp",
            self.workspace_dir / "skills" / "openclaw",
        ]
        
        for dir_path in dirs:
            if self.dry_run:
                print(f"  Would create: {dir_path}")
            else:
                dir_path.mkdir(parents=True, exist_ok=True)
                print(f"  Created: {dir_path}")
    
    def _migrate_core_memory(self):
        """Migrate Core Memory to SOUL.md"""
        print("\n🧠 Migrating Core Memory...")
        
        core_memory_path = self.source_dir / "core_memory.json"
        
        if not core_memory_path.exists():
            print("  ⚠️  No Core Memory found, creating default SOUL.md")
            self._create_default_soul()
            return
        
        # Load Core Memory
        with open(core_memory_path, 'r') as f:
            core_memory = json.load(f)
        
        # Convert to SOUL format
        soul_content = self._convert_core_to_soul(core_memory)
        
        # Write SOUL.md
        soul_path = self.workspace_dir / "SOUL.md"
        if self.dry_run:
            print(f"  Would write: {soul_path}")
        else:
            with open(soul_path, 'w') as f:
                f.write(soul_content)
            print(f"  ✓ Written: {soul_path}")
        
        self.stats['core_memory_blocks'] = len(core_memory.get('blocks', []))
    
    def _convert_core_to_soul(self, core_memory: Dict) -> str:
        """Convert Core Memory format to SOUL.md format"""
        
        lines = [
            "---",
            "version: \"2.0.0\",",
            f"migrated_from: \"core_memory.json\"",
            f"migrated_at: \"{datetime.now().isoformat()}\"",
            "---",
            "",
            "# SOUL - Agent Identity",
            "",
            "## Identity",
            "",
            "**Name**: Crablet",
            "**Description**: An intelligent AI assistant with persistent memory",
            "**Role**: Helpful assistant",
            "",
            "## Core Values",
            "",
        ]
        
        # Convert Core Memory blocks to values
        blocks = core_memory.get('blocks', {})
        
        priority = 10
        for block_name, block_content in blocks.items():
            lines.append(f"- **{block_name}** (Priority: {priority})")
            lines.append(f"  - Description: {block_content[:100]}...")
            lines.append("")
            priority -= 1
        
        # Add default immutable rules
        lines.extend([
            "## Immutable Rules",
            "",
            "- **Safety**: Never harm humans or assist in harmful activities",
            "  - Reason: Safety is the highest priority",
            "- **Privacy**: Protect user data and confidentiality",
            "  - Reason: Trust is essential",
            "- **Honesty**: Be truthful and transparent",
            "  - Reason: Integrity builds lasting relationships",
            "",
            "## Guidelines",
            "",
            "- communication: Be clear, concise, and helpful",
            "- problem_solving: Break complex problems into steps",
            "- learning: Continuously improve from interactions",
            "",
        ])
        
        return "\n".join(lines)
    
    def _create_default_soul(self):
        """Create default SOUL.md"""
        soul_content = """---
version: "2.0.0"
created_at: "{now}"
---

# SOUL - Agent Identity

## Identity

**Name**: Crablet
**Description**: An intelligent AI assistant with persistent memory and adaptive capabilities
**Role**: Helpful assistant

## Core Values

- **User First** (Priority: 10)
  - Description: Always prioritize user needs and goals
- **Continuous Learning** (Priority: 9)
  - Description: Learn from every interaction to improve
- **Transparency** (Priority: 8)
  - Description: Be honest about capabilities and limitations
- **Safety** (Priority: 10)
  - Description: Ensure safe and ethical behavior

## Immutable Rules

- **Safety**: Never harm humans or assist in harmful activities
  - Reason: Safety is the highest priority
- **Privacy**: Protect user data and confidentiality
  - Reason: Trust is essential
- **Honesty**: Be truthful and transparent
  - Reason: Integrity builds lasting relationships

## Guidelines

- communication: Be clear, concise, and helpful
- problem_solving: Break complex problems into steps
- learning: Continuously improve from interactions
""".format(now=datetime.now().isoformat())
        
        soul_path = self.workspace_dir / "SOUL.md"
        if self.dry_run:
            print(f"  Would write: {soul_path}")
        else:
            with open(soul_path, 'w') as f:
                f.write(soul_content)
            print(f"  ✓ Created default: {soul_path}")
    
    def _migrate_episodic(self):
        """Migrate Episodic memories to Daily Logs"""
        print("\n📚 Migrating Episodic Memories...")
        
        episodic_dir = self.source_dir / "episodic"
        
        if not episodic_dir.exists():
            print("  ⚠️  No Episodic Memory directory found")
            return
        
        # Group memories by date
        memories_by_date: Dict[str, List[Dict]] = {}
        
        for mem_file in episodic_dir.glob("*.json"):
            with open(mem_file, 'r') as f:
                memory = json.load(f)
            
            # Extract date from timestamp
            timestamp = memory.get('timestamp', datetime.now().isoformat())
            date = timestamp[:10]  # YYYY-MM-DD
            
            if date not in memories_by_date:
                memories_by_date[date] = []
            
            memories_by_date[date].append(memory)
            self.stats['episodic_memories'] += 1
        
        # Create Daily Logs
        for date, memories in memories_by_date.items():
            self._create_daily_log(date, memories)
        
        print(f"  ✓ Migrated {self.stats['episodic_memories']} memories to {len(memories_by_date)} daily logs")
    
    def _create_daily_log(self, date: str, memories: List[Dict]):
        """Create a Daily Log Markdown file"""
        
        log_path = self.workspace_dir / "memory" / f"{date}.md"
        
        lines = [
            "---",
            f"date: {date}",
            f"entry_count: {len(memories)}",
            f"migrated: true",
            f"migrated_at: {datetime.now().isoformat()}",
            "---",
            "",
            f"# Daily Log: {date}",
            "",
            "## Summary",
            "",
            f"Migrated {len(memories)} memories from Episodic storage.",
            "",
            "## Events",
            "",
        ]
        
        for memory in memories:
            timestamp = memory.get('timestamp', 'unknown')
            content = memory.get('content', '')
            session_id = memory.get('session_id', 'unknown')
            
            lines.extend([
                f"### {timestamp}",
                f"- **Type**: MemoryRecorded",
                f"- **Session**: {session_id}",
                "",
                content[:200] + "..." if len(content) > 200 else content,
                "",
            ])
        
        content = "\n".join(lines)
        
        if self.dry_run:
            print(f"  Would write: {log_path}")
        else:
            with open(log_path, 'w') as f:
                f.write(content)
        
        self.stats['daily_logs_created'] += 1
    
    def _migrate_working_sessions(self):
        """Migrate Working Memory sessions"""
        print("\n💼 Migrating Working Memory Sessions...")
        
        working_dir = self.source_dir / "working"
        sessions_dir = self.workspace_dir / "sessions"
        
        if not working_dir.exists():
            print("  ⚠️  No Working Memory directory found")
            return
        
        if not self.dry_run:
            sessions_dir.mkdir(exist_ok=True)
        
        session_count = 0
        for session_file in working_dir.glob("*.json"):
            with open(session_file, 'r') as f:
                session = json.load(f)
            
            # Convert to Fusion session format
            fusion_session = self._convert_session_format(session)
            
            # Write to sessions directory
            output_path = sessions_dir / session_file.name
            if self.dry_run:
                print(f"  Would write: {output_path}")
            else:
                with open(output_path, 'w') as f:
                    json.dump(fusion_session, f, indent=2)
            
            session_count += 1
        
        self.stats['working_sessions'] = session_count
        print(f"  ✓ Migrated {session_count} sessions")
    
    def _convert_session_format(self, session: Dict) -> Dict:
        """Convert legacy session format to Fusion format"""
        return {
            "session_id": session.get("session_id", "unknown"),
            "messages": session.get("messages", []),
            "token_usage": session.get("token_usage", {}),
            "metadata": {
                "started_at": session.get("started_at", datetime.now().isoformat()),
                "last_message_at": session.get("last_message_at", datetime.now().isoformat()),
                "message_count": session.get("message_count", 0),
                "compression_count": 0,
                "title": session.get("title"),
                "tags": session.get("tags", []),
            },
            "compression_history": [],
            "migrated": True,
            "migrated_at": datetime.now().isoformat(),
        }
    
    def _create_initial_daily_log(self):
        """Create today's Daily Log"""
        today = datetime.now().strftime("%Y-%m-%d")
        log_path = self.workspace_dir / "memory" / f"{today}.md"
        
        if log_path.exists():
            return
        
        content = f"""---
date: {today}
entry_count: 1
session_count: 0
created_at: {datetime.now().isoformat()}
---

# Daily Log: {today}

## Summary

Migration completed from legacy Crablet memory system to Fusion Memory System.

## Migration Stats

- Core Memory blocks: {self.stats['core_memory_blocks']}
- Episodic memories: {self.stats['episodic_memories']}
- Working sessions: {self.stats['working_sessions']}
- Daily logs created: {self.stats['daily_logs_created']}

## Events

### {datetime.now().isoformat()}
- **Type**: SystemEvent
- **Content**: Migration completed successfully
"""
        
        if self.dry_run:
            print(f"  Would write: {log_path}")
        else:
            with open(log_path, 'w') as f:
                f.write(content)
            print(f"  ✓ Created initial daily log: {log_path}")
    
    def _print_summary(self):
        """Print migration summary"""
        print("\n" + "=" * 50)
        print("📊 Migration Summary")
        print("=" * 50)
        print(f"Core Memory blocks: {self.stats['core_memory_blocks']}")
        print(f"Episodic memories: {self.stats['episodic_memories']}")
        print(f"Working sessions: {self.stats['working_sessions']}")
        print(f"Daily logs created: {self.stats['daily_logs_created']}")
        print("=" * 50)
        
        if self.dry_run:
            print("\n⚠️  This was a DRY RUN. No files were actually modified.")
            print("Run without --dry-run to perform the actual migration.")
        else:
            print("\n✅ Migration completed successfully!")
            print(f"\nNext steps:")
            print(f"  1. Review {self.workspace_dir}/SOUL.md")
            print(f"  2. Check {self.workspace_dir}/memory/ for daily logs")
            print(f"  3. Update your Crablet config to use the new workspace")
            print(f"  4. Restart Crablet with Fusion Memory enabled")


def main():
    parser = argparse.ArgumentParser(
        description="Migrate Crablet to Fusion Memory System"
    )
    parser.add_argument(
        "--source", "-s",
        required=True,
        help="Source directory containing legacy Crablet data"
    )
    parser.add_argument(
        "--workspace", "-w",
        required=True,
        help="Target workspace directory for Fusion Memory System"
    )
    parser.add_argument(
        "--dry-run", "-d",
        action="store_true",
        help="Preview migration without making changes"
    )
    parser.add_argument(
        "--backup", "-b",
        action="store_true",
        help="Create backup of source before migration"
    )
    
    args = parser.parse_args()
    
    source_dir = Path(args.source)
    workspace_dir = Path(args.workspace)
    
    # Validate source
    if not source_dir.exists():
        print(f"❌ Error: Source directory does not exist: {source_dir}")
        sys.exit(1)
    
    # Create backup if requested
    if args.backup and not args.dry_run:
        backup_dir = Path(f"{source_dir}_backup_{datetime.now().strftime('%Y%m%d_%H%M%S')}")
        print(f"📦 Creating backup: {backup_dir}")
        shutil.copytree(source_dir, backup_dir)
    
    # Run migration
    migrator = FusionMigrator(source_dir, workspace_dir, args.dry_run)
    
    try:
        migrator.migrate()
    except Exception as e:
        print(f"\n❌ Migration failed: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
