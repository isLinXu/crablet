import React, { useState, useCallback } from 'react';
import { Search, Brain, Sparkles, Tag, User } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Card, CardContent } from '@/components/ui/Card';
import { skillService } from '@/services/skillService';
import type { SemanticSearchResult } from '@/types/domain';
import toast from 'react-hot-toast';

interface SemanticSearchProps {
  onSelectSkill?: (skillName: string) => void;
  onRunSkill?: (skillName: string) => void;
}

export const SemanticSearch: React.FC<SemanticSearchProps> = ({ 
  onSelectSkill, 
  onRunSkill 
}) => {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SemanticSearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [hasSearched, setHasSearched] = useState(false);
  const [isFallback, setIsFallback] = useState(false);

  const handleSearch = useCallback(async () => {
    if (!query.trim()) {
      toast.error('请输入搜索关键词');
      return;
    }

    setLoading(true);
    setHasSearched(true);
    
    try {
      const res = await skillService.semanticSearch(query.trim(), 10, 0.3);
      setResults(res.results || []);
      setIsFallback(res.status === 'fallback');
      
      if (res.results?.length === 0) {
        toast('未找到匹配的技能，请尝试其他关键词');
      }
    } catch {
      toast.error('搜索失败，请稍后重试');
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, [query]);

  const getMatchTypeIcon = (matchType: string) => {
    switch (matchType) {
      case 'semantic':
        return <Brain className="w-4 h-4 text-purple-500" />;
      case 'hybrid':
        return <Sparkles className="w-4 h-4 text-amber-500" />;
      default:
        return <Tag className="w-4 h-4 text-blue-500" />;
    }
  };

  const getMatchTypeLabel = (matchType: string) => {
    switch (matchType) {
      case 'semantic':
        return '语义匹配';
      case 'hybrid':
        return '混合匹配';
      default:
        return '关键词匹配';
    }
  };

  const getScoreColor = (score: number) => {
    if (score >= 0.8) return 'text-green-600';
    if (score >= 0.5) return 'text-yellow-600';
    return 'text-gray-500';
  };

  return (
    <div className="space-y-4">
      <div className="flex gap-2">
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="描述你想要的功能，例如：数据分析、文件处理、网络请求..."
          className="flex-1"
          onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
        />
        <Button 
          onClick={handleSearch} 
          loading={loading}
          className="flex items-center gap-2"
        >
          <Search className="w-4 h-4" />
          语义搜索
        </Button>
      </div>

      {isFallback && hasSearched && (
        <div className="text-xs text-amber-600 bg-amber-50 dark:bg-amber-900/20 p-2 rounded">
          语义搜索服务暂时不可用，已切换到关键词搜索模式
        </div>
      )}

      {hasSearched && !loading && results.length > 0 && (
        <div className="text-sm text-gray-500">
          找到 {results.length} 个相关技能
        </div>
      )}

      <div className="space-y-3 max-h-96 overflow-y-auto">
        {results.map((result, index) => (
          <Card 
            key={`${result.skill_name}-${index}`}
            className="hover:shadow-md transition-shadow cursor-pointer"
            onClick={() => onSelectSkill?.(result.skill_name)}
          >
            <CardContent className="p-4">
              <div className="flex items-start justify-between gap-3">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-1">
                    <h4 className="font-semibold text-gray-900 dark:text-gray-100 truncate">
                      {result.skill_name}
                    </h4>
                    <span className="text-xs text-gray-400">v{result.version}</span>
                  </div>
                  
                  <p className="text-sm text-gray-600 dark:text-gray-300 line-clamp-2 mb-2">
                    {result.description}
                  </p>
                  
                  <div className="flex items-center gap-4 text-xs text-gray-500">
                    <div className="flex items-center gap-1">
                      {getMatchTypeIcon(result.match_type)}
                      <span>{getMatchTypeLabel(result.match_type)}</span>
                    </div>
                    
                    <div className="flex items-center gap-1">
                      <User className="w-3 h-3" />
                      <span>{result.author}</span>
                    </div>
                    
                    {result.category && result.category !== 'General' && (
                      <span className="px-2 py-0.5 bg-gray-100 dark:bg-gray-800 rounded">
                        {result.category}
                      </span>
                    )}
                  </div>
                </div>
                
                <div className="flex flex-col items-end gap-2">
                  <div className={`text-lg font-bold ${getScoreColor(result.similarity_score)}`}>
                    {(result.similarity_score * 100).toFixed(0)}%
                  </div>
                  
                  {onRunSkill && (
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={(e) => {
                        e.stopPropagation();
                        onRunSkill(result.skill_name);
                      }}
                    >
                      运行
                    </Button>
                  )}
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      {hasSearched && !loading && results.length === 0 && (
        <div className="text-center py-8 text-gray-500">
          <Search className="w-12 h-12 mx-auto mb-3 text-gray-300" />
          <p>未找到匹配的技能</p>
          <p className="text-sm mt-1">尝试使用不同的描述或更通用的关键词</p>
        </div>
      )}
    </div>
  );
};
