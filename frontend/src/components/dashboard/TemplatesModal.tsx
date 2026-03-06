import React, { useState, useEffect } from 'react';
import { Modal } from '../ui/Modal';
import client from '../../api/client'; // Assuming client is exported from api/client or similar
import { Play } from 'lucide-react';
import toast from 'react-hot-toast';

interface Template {
    id: string;
    name: string;
    description: string;
    created_at: number;
}

interface TemplatesModalProps {
    isOpen: boolean;
    onClose: () => void;
}

export const TemplatesModal: React.FC<TemplatesModalProps> = ({ isOpen, onClose }) => {
    const [templates, setTemplates] = useState<Template[]>([]);
    const [loading, setLoading] = useState(true);

    const fetchTemplates = async () => {
        try {
            const res = await client.get('/api/swarm/templates');
            setTemplates(res.data.templates || []);
        } catch (e) {
            console.error(e);
            toast.error('Failed to load templates');
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        if (isOpen) fetchTemplates();
    }, [isOpen]);

    const handleInstantiate = async (id: string) => {
        const goal = prompt("Enter a goal for this new task:");
        if (!goal) return;

        try {
            await client.post(`/api/swarm/templates/${id}/instantiate`, { goal });
            toast.success('Template instantiated!');
            onClose();
            // Trigger refresh of graphs?
            window.location.reload(); // Simple refresh for now
        } catch (e) {
            toast.error('Failed to instantiate template');
        }
    };

    return (
        <Modal isOpen={isOpen} onClose={onClose} title="Task Templates">
            <div className="space-y-4">
                {loading ? (
                    <p>Loading...</p>
                ) : templates.length === 0 ? (
                    <p className="text-gray-500">No templates found.</p>
                ) : (
                    <div className="grid gap-4">
                        {templates.map(t => (
                            <div key={t.id} className="border p-4 rounded-lg flex justify-between items-center dark:border-gray-700">
                                <div>
                                    <h3 className="font-bold dark:text-white">{t.name}</h3>
                                    <p className="text-sm text-gray-600 dark:text-gray-400">{t.description}</p>
                                    <p className="text-xs text-gray-400 mt-1">Created: {new Date(t.created_at * 1000).toLocaleDateString()}</p>
                                </div>
                                <button 
                                    onClick={() => handleInstantiate(t.id)}
                                    className="p-2 bg-blue-50 text-blue-600 rounded-full hover:bg-blue-100"
                                    title="Use Template"
                                >
                                    <Play size={20} />
                                </button>
                            </div>
                        ))}
                    </div>
                )}
            </div>
        </Modal>
    );
};

interface SaveTemplateModalProps {
    isOpen: boolean;
    onClose: () => void;
    graphId: string;
}

export const SaveTemplateModal: React.FC<SaveTemplateModalProps> = ({ isOpen, onClose, graphId }) => {
    const [name, setName] = useState('');
    const [description, setDescription] = useState('');
    const [saving, setSaving] = useState(false);

    const handleSave = async (e: React.FormEvent) => {
        e.preventDefault();
        setSaving(true);
        try {
            await client.post('/api/swarm/templates', {
                name,
                description,
                graph_id: graphId
            });
            toast.success('Template saved!');
            onClose();
        } catch (e) {
            toast.error('Failed to save template');
        } finally {
            setSaving(false);
        }
    };

    return (
        <Modal isOpen={isOpen} onClose={onClose} title="Save as Template">
            <form onSubmit={handleSave} className="space-y-4">
                <div>
                    <label className="block text-sm font-medium mb-1">Name</label>
                    <input 
                        className="w-full border rounded p-2 dark:bg-gray-700 dark:border-gray-600"
                        value={name} 
                        onChange={e => setName(e.target.value)} 
                        required 
                    />
                </div>
                <div>
                    <label className="block text-sm font-medium mb-1">Description</label>
                    <textarea 
                        className="w-full border rounded p-2 dark:bg-gray-700 dark:border-gray-600"
                        value={description} 
                        onChange={e => setDescription(e.target.value)} 
                        required 
                    />
                </div>
                <div className="flex justify-end gap-2">
                    <button type="button" onClick={onClose} className="px-4 py-2 text-gray-600">Cancel</button>
                    <button type="submit" disabled={saving} className="px-4 py-2 bg-blue-600 text-white rounded">
                        {saving ? 'Saving...' : 'Save'}
                    </button>
                </div>
            </form>
        </Modal>
    );
};
