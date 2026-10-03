export type Provider={name:string;protocol:'openai'|'anthropic';base_url:string;key:string;model:string};
export type Project={id:string;name:string;path:string};
export type Preview={path:string;before:string;after:string;existed:boolean};
export type Call={id:string;function:{name:string;arguments:string};preview?:Preview};
export type Pending={grant:string;calls:Call[]};
export type AgentEvent={id:string;type:string;text?:string;name?:string;arguments?:string;status?:string;result?:any;complete?:boolean;animate?:boolean;preview?:Preview};
export type Chat={id:string;title:string;project_id:string|null;permission:'ask'|'read'|'project';events:AgentEvent[];pending:Pending|null};
