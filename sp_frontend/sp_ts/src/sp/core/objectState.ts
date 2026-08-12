export type MapObjectPresence = 'perceived' | 'remembered' | 'destroyed';

export interface ObjectState {
    id : string;
    player : string;
    name : string;
    class : string;
    subclass : string;
    template : string;
    state : string;
    activity? : string;
    prevstate : string;
    groups : Array<string>;
    x : integer;
    y : integer;
    prevX? : integer;
    prevY? : integer;
    vision : integer | null;
    image : string;
    portrait? : string | null;
    hsl? : number[];
    work_done?: number;
    total_work?: number;
    work_per_sec?: number;
    action_id?: number;
    action_duration_ms?: number;
    action_elapsed_ms?: number;
    perceptionObserver?: boolean;
    presence?: MapObjectPresence;
    op? : string;
    updateAttr?: string;
    eventType? : string;
}
