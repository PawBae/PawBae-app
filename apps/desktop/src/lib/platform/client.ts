// PlatformClient 注入点——line-c §3 说的「一行 DI 切换」发生在这里：
// W5-6 串门 UI 全部对着 MockPlatformClient 开发；W7（本次）换入真实实现。
// 消费方（visitStore/Main）只认 PlatformClient 契约面，零改动。
//
// 导出类型用具体类：start()/dispose() 是宿主接线钩子（App 挂载时调用），
// 不属于 §2 冻结契约——contract 消费方仍应以 PlatformClient 类型收窄。

import { createSupabasePlatformClient, type SupabasePlatformClient } from './supabase-client';

export const platformClient: SupabasePlatformClient = createSupabasePlatformClient();
