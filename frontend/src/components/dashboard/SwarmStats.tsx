import React, { useEffect, useState } from 'react';
import type { SwarmStatsData } from '@/types/domain';
import { Card } from '../ui/Card';
import { Activity, CheckCircle, XCircle, Clock, BarChart2 } from 'lucide-react';
import { dashboardService } from '@/services/dashboardService';

export const SwarmStats: React.FC = () => {
    const [stats, setStats] = useState<SwarmStatsData | null>(null);
    const [loading, setLoading] = useState(true);

    const fetchStats = async () => {
        try {
            const data = await dashboardService.getSwarmStats();
            setStats(data);
        } catch (e) {
            console.error("Failed to fetch swarm stats", e);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        fetchStats();
        const interval = setInterval(fetchStats, 10000); // Poll every 10s
        return () => clearInterval(interval);
    }, []);

    if (loading || !stats) {
        return <div className="animate-pulse h-32 bg-gray-100 dark:bg-gray-800 rounded-lg"></div>;
    }

    return (
        <div className="grid grid-cols-2 md:grid-cols-5 gap-4 mb-6">
            <StatCard 
                title="Total Tasks" 
                value={stats.total_tasks} 
                icon={BarChart2} 
                color="text-gray-600 dark:text-gray-300"
                bgColor="bg-gray-50 dark:bg-gray-800" 
            />
            <StatCard 
                title="Active" 
                value={stats.active} 
                icon={Activity} 
                color="text-blue-600 dark:text-blue-400" 
                bgColor="bg-blue-50 dark:bg-blue-900/20"
            />
            <StatCard 
                title="Completed" 
                value={stats.completed} 
                icon={CheckCircle} 
                color="text-green-600 dark:text-green-400"
                bgColor="bg-green-50 dark:bg-green-900/20"
            />
            <StatCard 
                title="Failed" 
                value={stats.failed} 
                icon={XCircle} 
                color="text-red-600 dark:text-red-400"
                bgColor="bg-red-50 dark:bg-red-900/20"
            />
            <StatCard 
                title="Avg Duration" 
                value={`${stats.avg_duration_sec.toFixed(1)}s`} 
                icon={Clock} 
                color="text-purple-600 dark:text-purple-400"
                bgColor="bg-purple-50 dark:bg-purple-900/20"
            />
        </div>
    );
};

const StatCard: React.FC<{ 
    title: string; 
    value: string | number; 
    icon: React.ElementType; 
    color: string;
    bgColor: string;
}> = ({ title, value, icon: Icon, color, bgColor }) => (
    <Card className={`p-4 flex items-center gap-4 ${bgColor} border-none`}>
        <div className={`p-2 rounded-full bg-white dark:bg-gray-800 ${color} shadow-sm`}>
            <Icon size={20} />
        </div>
        <div>
            <p className="text-xs text-gray-500 dark:text-gray-400 font-medium uppercase tracking-wide">{title}</p>
            <p className="text-xl font-bold text-gray-900 dark:text-gray-100">{value}</p>
        </div>
    </Card>
);
