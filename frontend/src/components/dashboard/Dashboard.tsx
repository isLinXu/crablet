import React, { useEffect, useState } from 'react';
import type { DashboardStats } from '@/types/domain';
import { Card } from '../ui/Card';
import { Skeleton } from '../ui/Skeleton';
import { SwarmGraph } from './SwarmGraph';
import { dashboardService } from '@/services/dashboardService';

export const Dashboard: React.FC = () => {
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchStats = async () => {
      try {
        const data = await dashboardService.getDashboardStats();
        setStats(data);
      } catch (err) {
        setError('Failed to load dashboard data');
        console.error(err);
      } finally {
        setLoading(false);
      }
    };

    fetchStats();
  }, []);

  if (loading) {
    return (
      <div className="p-6 space-y-6">
        <h1 className="text-2xl font-bold">System Dashboard</h1>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
          <Skeleton className="h-32" />
          <Skeleton className="h-32" />
          <Skeleton className="h-32" />
        </div>
      </div>
    );
  }

  if (error) {
    return <div className="p-6 text-red-500">{error}</div>;
  }

  return (
    <div className="p-6 space-y-6 h-full overflow-y-auto">
      <h1 className="text-2xl font-bold text-gray-800 dark:text-gray-100">System Dashboard</h1>
      
      {/* Key Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        <Card className="p-4 flex flex-col items-center justify-center bg-blue-50 dark:bg-blue-900/20 border-blue-100 dark:border-blue-800">
          <span className="text-4xl font-bold text-blue-600 dark:text-blue-400">{stats?.skills_count}</span>
          <span className="text-sm text-gray-600 dark:text-gray-400 mt-2">Installed Skills</span>
        </Card>
        
        <Card className="p-4 flex flex-col items-center justify-center bg-green-50 dark:bg-green-900/20 border-green-100 dark:border-green-800">
          <span className="text-4xl font-bold text-green-600 dark:text-green-400">{stats?.active_tasks}</span>
          <span className="text-sm text-gray-600 dark:text-gray-400 mt-2">Active Tasks</span>
        </Card>
        
        <Card className="p-4 flex flex-col items-center justify-center bg-purple-50 dark:bg-purple-900/20 border-purple-100 dark:border-purple-800">
          <span className="text-xl font-bold text-purple-600 dark:text-purple-400">{stats?.system_load}</span>
          <span className="text-sm text-gray-600 dark:text-gray-400 mt-2">System Load</span>
        </Card>
      </div>

      {/* Swarm Graph Visualization */}
      <SwarmGraph />

      {/* Skills List */}
      <Card className="p-6">
        <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-gray-100">Installed Skills</h2>
        <div className="overflow-x-auto">
          <table className="w-full text-left">
            <thead>
              <tr className="border-b dark:border-gray-700 text-gray-600 dark:text-gray-400">
                <th className="pb-3 font-medium">Name</th>
                <th className="pb-3 font-medium">Version</th>
                <th className="pb-3 font-medium">Description</th>
              </tr>
            </thead>
            <tbody className="divide-y dark:divide-gray-700">
              {stats?.skills.map((skill) => (
                <tr key={skill.name} className="group hover:bg-gray-50 dark:hover:bg-gray-800/50">
                  <td className="py-3 font-medium text-gray-800 dark:text-gray-200">{skill.name}</td>
                  <td className="py-3 text-gray-600 dark:text-gray-400 font-mono text-sm">{skill.version}</td>
                  <td className="py-3 text-gray-600 dark:text-gray-400">{skill.description}</td>
                </tr>
              ))}
              {stats?.skills.length === 0 && (
                <tr>
                  <td colSpan={3} className="py-4 text-center text-gray-500">No skills installed</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </Card>
    </div>
  );
};
