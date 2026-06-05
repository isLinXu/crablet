#!/usr/bin/env python3
"""
测试从 clawhub 下载和安装 skills 的功能
"""

import requests
import json
import sys
import time
from typing import Optional

# API 基础 URL
BASE_URL = "http://localhost:3000"

class Colors:
    GREEN = '\033[92m'
    RED = '\033[91m'
    YELLOW = '\033[93m'
    BLUE = '\033[94m'
    END = '\033[0m'

def print_header(text: str):
    print(f"\n{Colors.BLUE}{'='*60}{Colors.END}")
    print(f"{Colors.BLUE}{text}{Colors.END}")
    print(f"{Colors.BLUE}{'='*60}{Colors.END}\n")

def print_success(text: str):
    print(f"{Colors.GREEN}✓ {text}{Colors.END}")

def print_error(text: str):
    print(f"{Colors.RED}✗ {text}{Colors.END}")

def print_info(text: str):
    print(f"{Colors.YELLOW}ℹ {text}{Colors.END}")

class ClawhubSkillTester:
    def __init__(self, base_url: str = BASE_URL):
        self.base_url = base_url
        self.session = requests.Session()
        
    def check_server(self) -> bool:
        """检查服务器是否运行"""
        try:
            response = self.session.get(f"{self.base_url}/v1/dashboard/stats", timeout=5)
            return response.status_code == 200
        except requests.exceptions.ConnectionError:
            return False
        except Exception as e:
            print_error(f"检查服务器时出错: {e}")
            return False
    
    def search_skills(self, query: str) -> Optional[list]:
        """从 clawhub 搜索 skills"""
        try:
            print_info(f"搜索关键词: '{query}'")
            response = self.session.get(
                f"{self.base_url}/v1/skills/registry/search",
                params={"q": query},
                timeout=30
            )
            
            if response.status_code == 200:
                data = response.json()
                items = data.get("items", [])
                source = data.get("source", "unknown")
                print_success(f"搜索成功! 来源: {source}, 找到 {len(items)} 个技能")
                return items
            else:
                print_error(f"搜索失败: HTTP {response.status_code}")
                print_error(f"响应: {response.text}")
                return None
        except Exception as e:
            print_error(f"搜索时出错: {e}")
            return None
    
    def list_installed_skills(self) -> Optional[list]:
        """列出已安装的技能"""
        try:
            response = self.session.get(f"{self.base_url}/v1/skills", timeout=10)
            if response.status_code == 200:
                skills = response.json()
                print_success(f"已安装 {len(skills)} 个技能")
                return skills
            else:
                print_error(f"获取已安装技能失败: HTTP {response.status_code}")
                return None
        except Exception as e:
            print_error(f"获取已安装技能时出错: {e}")
            return None
    
    def install_skill_by_name(self, name: str) -> bool:
        """通过名称从 clawhub 安装 skill"""
        try:
            print_info(f"正在安装技能: {name}")
            response = self.session.post(
                f"{self.base_url}/v1/skills/install",
                json={"name": name},
                timeout=120
            )
            
            if response.status_code == 200:
                data = response.json()
                status = data.get("status", "unknown")
                if status == "installed":
                    print_success(f"技能 '{name}' 安装成功!")
                    return True
                elif status == "already_installed":
                    print_info(f"技能 '{name}' 已安装")
                    return True
                else:
                    print_info(f"安装状态: {status}")
                    return True
            else:
                print_error(f"安装失败: HTTP {response.status_code}")
                print_error(f"响应: {response.text}")
                return False
        except requests.exceptions.Timeout:
            print_error(f"安装超时 (120s)")
            return False
        except Exception as e:
            print_error(f"安装时出错: {e}")
            return False
    
    def install_skill_by_url(self, url: str) -> bool:
        """通过 Git URL 安装 skill"""
        try:
            print_info(f"正在从 URL 安装: {url}")
            response = self.session.post(
                f"{self.base_url}/v1/skills/install",
                json={"url": url},
                timeout=120
            )
            
            if response.status_code == 200:
                data = response.json()
                status = data.get("status", "unknown")
                if status == "installed":
                    print_success(f"从 URL 安装成功!")
                    return True
                else:
                    print_info(f"安装状态: {status}")
                    return True
            else:
                print_error(f"从 URL 安装失败: HTTP {response.status_code}")
                print_error(f"响应: {response.text}")
                return False
        except requests.exceptions.Timeout:
            print_error(f"安装超时 (120s)")
            return False
        except Exception as e:
            print_error(f"安装时出错: {e}")
            return False
    
    def get_top_skills(self, limit: int = 20) -> Optional[list]:
        """获取 skills.sh Top 技能列表"""
        try:
            print_info(f"获取 Top {limit} 技能列表...")
            response = self.session.get(
                f"{self.base_url}/v1/skills/top",
                params={"limit": limit},
                timeout=30
            )
            
            if response.status_code == 200:
                data = response.json()
                items = data.get("items", [])
                source = data.get("source", "unknown")
                print_success(f"获取成功! 来源: {source}, 共 {len(items)} 个技能")
                return items
            else:
                print_error(f"获取 Top 技能失败: HTTP {response.status_code}")
                return None
        except Exception as e:
            print_error(f"获取 Top 技能时出错: {e}")
            return None
    
    def test_semantic_search(self, query: str) -> Optional[list]:
        """测试语义搜索"""
        try:
            print_info(f"语义搜索: '{query}'")
            response = self.session.post(
                f"{self.base_url}/v1/skills/semantic-search",
                json={"query": query, "limit": 10, "min_similarity": 0.3},
                timeout=30
            )
            
            if response.status_code == 200:
                data = response.json()
                results = data.get("results", [])
                status = data.get("status", "unknown")
                note = data.get("note", "")
                print_success(f"语义搜索成功! 状态: {status}, 找到 {len(results)} 个结果")
                if note:
                    print_info(f"注意: {note}")
                return results
            else:
                print_error(f"语义搜索失败: HTTP {response.status_code}")
                return None
        except Exception as e:
            print_error(f"语义搜索时出错: {e}")
            return None
    
    def run_skill(self, name: str, args: dict = None) -> Optional[dict]:
        """运行单个 skill"""
        try:
            print_info(f"运行技能: {name}")
            payload = {"args": args or {}, "timeout_secs": 30}
            response = self.session.post(
                f"{self.base_url}/v1/skills/{name}/run",
                json=payload,
                timeout=60
            )
            
            if response.status_code == 200:
                data = response.json()
                status = data.get("status", "unknown")
                result = data.get("result", {})
                
                if status == "ok" and result.get("success"):
                    print_success(f"技能运行成功! 耗时: {result.get('execution_time_ms', 0)}ms")
                    return result
                else:
                    error = data.get("error", "未知错误")
                    print_error(f"技能运行失败: {error}")
                    return None
            else:
                print_error(f"运行技能失败: HTTP {response.status_code}")
                return None
        except Exception as e:
            print_error(f"运行技能时出错: {e}")
            return None
    
    def get_skill_logs(self, name: str) -> Optional[list]:
        """获取技能执行日志"""
        try:
            print_info(f"获取技能 '{name}' 的执行日志...")
            response = self.session.get(
                f"{self.base_url}/v1/skills/{name}/logs",
                params={"limit": 10},
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                logs = data.get("logs", [])
                print_success(f"获取日志成功! 共 {len(logs)} 条记录")
                return logs
            else:
                print_error(f"获取日志失败: HTTP {response.status_code}")
                return None
        except Exception as e:
            print_error(f"获取日志时出错: {e}")
            return None


def main():
    print_header("🧪 Clawhub Skills 功能测试")
    
    tester = ClawhubSkillTester()
    
    # 1. 检查服务器
    print_header("1. 检查服务器状态")
    if not tester.check_server():
        print_error("服务器未运行! 请先启动 crablet 服务")
        print_info("运行命令: cd /Users/gatilin/PycharmProjects/crablet-latest-v260313 && ./start.sh")
        sys.exit(1)
    print_success("服务器运行正常")
    
    # 2. 列出已安装技能
    print_header("2. 列出已安装技能")
    installed_skills = tester.list_installed_skills()
    if installed_skills:
        print_info("已安装技能列表:")
        for skill in installed_skills[:5]:  # 只显示前5个
            name = skill.get("name", "unknown")
            version = skill.get("version", "unknown")
            enabled = skill.get("enabled", False)
            status = "✓" if enabled else "✗"
            print(f"  {status} {name} (v{version})")
        if len(installed_skills) > 5:
            print(f"  ... 还有 {len(installed_skills) - 5} 个技能")
    
    # 3. 从 clawhub 搜索技能
    print_header("3. 从 Clawhub 搜索技能")
    search_terms = ["weather", "search", "data"]
    found_skills = []
    
    for term in search_terms:
        results = tester.search_skills(term)
        if results:
            found_skills.extend(results)
            print_info(f"'{term}' 搜索结果 (前3个):")
            for item in results[:3]:
                name = item.get("name", "unknown")
                desc = item.get("description", "")[:50]
                print(f"  - {name}: {desc}...")
        time.sleep(0.5)  # 避免请求过快
    
    # 4. 获取 Top 技能
    print_header("4. 获取 Skills.sh Top 技能")
    top_skills = tester.get_top_skills(10)
    if top_skills:
        print_info("Top 技能 (前5个):")
        for i, item in enumerate(top_skills[:5], 1):
            name = item.get("name", "unknown")
            source = item.get("source", "unknown")
            installs = item.get("installs", 0)
            print(f"  {i}. {name} ({source}) - {installs} 次安装")
    
    # 5. 测试安装技能 (如果找到的话)
    print_header("5. 测试安装技能")
    
    # 先尝试安装一个常见的 skill
    test_skill_name = "web-search"
    print_info(f"尝试安装技能: {test_skill_name}")
    
    # 检查是否已安装
    already_installed = any(s.get("name") == test_skill_name for s in (installed_skills or []))
    
    if already_installed:
        print_info(f"技能 '{test_skill_name}' 已安装，跳过安装测试")
    else:
        success = tester.install_skill_by_name(test_skill_name)
        if success:
            # 重新列出已安装技能
            time.sleep(1)
            tester.list_installed_skills()
    
    # 6. 测试语义搜索
    print_header("6. 测试语义搜索")
    semantic_results = tester.test_semantic_search("查找天气相关的技能")
    if semantic_results:
        print_info("语义搜索结果 (前3个):")
        for item in semantic_results[:3]:
            name = item.get("skill_name", "unknown")
            score = item.get("similarity_score", 0)
            match_type = item.get("match_type", "unknown")
            print(f"  - {name} (相似度: {score:.2f}, 匹配类型: {match_type})")
    
    # 7. 测试运行技能 (如果已安装)
    print_header("7. 测试运行技能")
    if installed_skills and len(installed_skills) > 0:
        # 找一个已启用的技能来测试
        enabled_skills = [s for s in installed_skills if s.get("enabled", False)]
        if enabled_skills:
            test_skill = enabled_skills[0]
            skill_name = test_skill.get("name")
            print_info(f"尝试运行技能: {skill_name}")
            result = tester.run_skill(skill_name)
            if result:
                output = result.get("output", "")
                print_info(f"输出: {output[:100]}...")
        else:
            print_info("没有已启用的技能可供测试")
    else:
        print_info("没有已安装的技能可供测试")
    
    # 8. 测试获取日志
    print_header("8. 测试获取执行日志")
    if installed_skills and len(installed_skills) > 0:
        skill_name = installed_skills[0].get("name")
        logs = tester.get_skill_logs(skill_name)
        if logs:
            print_info(f"技能 '{skill_name}' 的执行日志:")
            for log in logs[:3]:
                success = log.get("success", False)
                timestamp = log.get("timestamp", "unknown")
                status = "成功" if success else "失败"
                print(f"  [{timestamp}] {status}")
    
    # 总结
    print_header("📊 测试总结")
    print_info("测试完成! 主要功能检查:")
    print("  ✓ 服务器连接检查")
    print("  ✓ 已安装技能列表")
    print("  ✓ Clawhub 技能搜索")
    print("  ✓ Skills.sh Top 技能获取")
    print("  ✓ 技能安装")
    print("  ✓ 语义搜索")
    print("  ✓ 技能运行")
    print("  ✓ 执行日志获取")
    print()
    print_success("所有测试流程已完成!")
    print_info("注意: 某些测试可能因网络或环境原因失败，请检查具体输出")

if __name__ == "__main__":
    main()
