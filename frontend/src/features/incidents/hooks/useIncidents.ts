import { useQuery } from '@tanstack/react-query';
import client from '../../../api/rpc';
import { IncidentsResponse } from '../types';

export const useIncidents = (limit: number = 50, interval: number = 5000) => {
  return useQuery({
    queryKey: ['incidents', limit],
    queryFn: async () => {
      const res = await client.analysis.incidents.$get({
        query: { limit: limit.toString() }
      });
      if (!res.ok) throw new Error('Failed to fetch incidents');
      return res.json() as Promise<IncidentsResponse>;
    },
    refetchInterval: interval,
  });
};
