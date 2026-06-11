export interface ObjectState {
    id : string;
    player : string;
    name : string;
    class : string;
    subclass : string;
    template : string;
    state : string;
    prevstate : string;
    groups : Array<string>;
    x : integer;
    y : integer;
    prevX? : integer;
    prevY? : integer;
    vision : integer;
    image : string;
    hsl? : number[];
    work_done?: number;
    total_work?: number;
    work_per_sec?: number;
    op? : string;
    updateAttr?: string;
    eventType? : string;
}
