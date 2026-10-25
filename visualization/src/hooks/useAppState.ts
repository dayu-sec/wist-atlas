import { useReducer } from 'react';
import type { AppState, AppAction } from '../types/domain';

const initialLayerVisibility = {
  resource: true,
  governance: true,
} as const;

const initialState: AppState = {
  graph: null,
  hostTopology: null,
  loading: false,
  error: null,
  selectedNodeId: null,
  layerVisibility: { ...initialLayerVisibility },
  searchQuery: '',
  layout: 'auto',
};

function reducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'FETCH_GRAPH_START':
      return { ...state, loading: true, error: null };
    case 'FETCH_GRAPH_SUCCESS':
      return { ...state, loading: false, graph: action.graph };
    case 'FETCH_HOST_TOPOLOGY_SUCCESS':
      return { ...state, hostTopology: action.hostTopology };
    case 'FETCH_GRAPH_ERROR':
      return { ...state, loading: false, error: action.error };
    case 'SELECT_NODE':
      return { ...state, selectedNodeId: action.nodeId };
    case 'TOGGLE_LAYER':
      return {
        ...state,
        layerVisibility: {
          ...state.layerVisibility,
          [action.layer]: !state.layerVisibility[action.layer],
        },
      };
    case 'SET_SEARCH':
      return { ...state, searchQuery: action.query };
    case 'SET_LAYOUT':
      return { ...state, layout: action.layout };
    default:
      return state;
  }
}

export function useAppState() {
  return useReducer(reducer, initialState);
}
