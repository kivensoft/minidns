use std::cell::Cell;
use std::collections::HashMap;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use mio::{Events, Interest, Poll, Token, net::UdpSocket};
use super::bufutil::*;
use super::dnsutil::*;

// dyndns 常量定义
const C_2023_01_01: u64            = 1672531200;                          // 动态dns更新的时间基数: 2023-01-01起到现在的秒数
const C_DNYDNS_MAGIC: &[u8]        = b"kdns";                             // 动态dns数据包魔数
const C_DYNDNS_MIN_LEN: usize      = 4 + 1 + 32 + 1 + 1 + 1 + 1 + 1 + 7;  // 动态dns数据包最小长度
const C_DYNDNS_PARAM_COUNT: usize  = 5;                                   // 动态dns参数数量
const C_DYNDNS_PARAM_DIGEST: usize = 1;
const C_DYNDNS_PARAM_ID: usize     = 2;
const C_DYNDNS_PARAM_HOST: usize   = 3;
const C_DYNDNS_PARAM_IP: usize     = 4;
const C_DYNDNS_TIME_RANGE: u64     = 60 * 10;                             // 动态dns更新时间允许的误差

// dnsserver 常量定义
const QUERY_TIMEOUT: u64          = 10;        // 查询超时时间(秒)
const CLEAR_QUERIES_INTERVAL: u64 = 10;        // 定期清理查询队列时间间隔(秒)
const MAX_FORWARD_COUNT: u8       = 10;        // 转发查询的最大跳转次数, 防止无限循环
const MAX_QUERIES_LEN: usize      = 4096;      // 队列允许的最大长度
const SERVER_TOKEN: Token         = Token(0);  // 监听服务的token
const UP_SERVER_TOKEN: Token      = Token(1);  // 向上级dns转发查询服务的token

// 待解析的查询项
struct QueryData {
    id      : u16,           // 来自dns查询请求的查询请求id
    addr    : SocketAddr,    // 来自dns查询请求的客户端地址
    question: DnsQuestion,   // 来自dns查询请求的查询条目
    forword : u16,           // 当前递归查询指向的上一级QueryData的id
    expire  : u64,           // 查询过期时间戳, Unix格式: 自1970-01-01至今的秒数
    count   : Cell<u8>,      // 当前的转发查询次数, 需要做一些限制, 否则有可能陷入死循环
}

type Query   = Rc<QueryData>;
type Queries = HashMap<u16, Query>;
type HOSTS   = HashMap<String, Ipv4Addr>;

pub struct DnsServer {
    socket     : UdpSocket,    // DNS服务socket
    up_socket  : UdpSocket,    // 上级dns连接地址
    poll       : Poll,         // DNS服务事件提取器
    queries    : Queries,      // 所有向上级发送的查询请求但尚未收到回复的连接信息
    curr_req_id: u16,          // 向上级DNS发送查询请求的当前请求id
    up_dns_addr: IpAddr,       // 上级dns服务器地址
    ttl        : u32,          // dns服务器回复的查询结果的生存时间
    hosts      : HOSTS,        // 本服务器可以解析的域名字典
    key        : String,       // 动态域名更新密钥
}

impl DnsServer {

    pub fn create(listen_addr: &str, up_dns_addr: &str, ttl: u32, key: &str) -> Result<DnsServer> {
        let socket = UdpSocket::bind(listen_addr.parse()?)?;
        log::info!("dns server startup {}, parent dns server {}", listen_addr, up_dns_addr);
        Ok(DnsServer {
            socket,
            up_socket: UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0))?,
            poll: Poll::new()?,
            queries: Queries::new(),
            curr_req_id: 0,
            up_dns_addr: up_dns_addr.parse()?,
            ttl,
            hosts: HOSTS::new(),
            key: key.to_string(),
        })
    }

    pub fn register_host(&mut self, host: &str, ip: &str) -> Result<()> {
        log::debug!("register local host: {} {}", host, ip);
        self.hosts.insert(host.to_string(), ip.parse()?);
        Ok(())
    }

    pub fn run(&mut self, event_capacity: usize) -> Result<()> {
        let mut req_buffer = BytePacketBuffer::new();
        let mut events = Events::with_capacity(event_capacity);
        let mut next_clear_time = now_of_unix() + CLEAR_QUERIES_INTERVAL;

        self.poll.registry().register(&mut self.socket, SERVER_TOKEN, Interest::READABLE)?;
        self.poll.registry().register(&mut self.up_socket, UP_SERVER_TOKEN, Interest::READABLE)?;

        loop {
            self.poll.poll(&mut events, None)?;
            
            for event in events.iter() {
                match event.token() {
                    SERVER_TOKEN => self.server_recv(&mut req_buffer)?,
                    UP_SERVER_TOKEN => self.client_recv(&mut req_buffer)?,
                    _ => {},
                }
            }

            // 定时清理待查询队列
            let now = now_of_unix();
            if next_clear_time < now {
                self.clear_queries_of_timeout();
                next_clear_time = now + CLEAR_QUERIES_INTERVAL;
            }
        }
    }

    fn server_recv(&mut self, req_buffer: &mut BytePacketBuffer) -> Result<()> {
        loop {
            req_buffer.pos = 0;
            let (packet_size, source_address) = match self.socket.recv_from(&mut req_buffer.buf) {
                Ok((packet_size, source_address)) => (packet_size, source_address),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            };
            req_buffer.len = packet_size;

            // 处理动态dns更新
            match self.dyn_dns(req_buffer, &source_address) {
                Ok(true) => continue,
                Ok(false) => {},
                Err(e) => log::error!("dyndns server error: {}", e),
            }

            match DnsPacket::from_buffer(req_buffer) {
                Ok(ref mut request) => match request.questions.pop() {
                    Some(question) => {
                        // 处理dns请求
                        let query = Query::new(QueryData {
                            id: request.header.id,
                            addr: source_address,
                            question: question,
                            forword: 0,
                            expire: expire_of_unix(),
                            count: Cell::new(0),
                        });
        
                        if let Err(e) = self.handle_query(&query) {
                            log::error!("failed to process query request: {}", e);
                        }
                    },
                    None => log::error!("serve_recv no question found in the received request package"),
                },
                Err(e) => log::error!("serve_recv data format error: {}", e),
            }
        }

        Ok(())
    }

    fn client_recv(&mut self, req_buffer: &mut BytePacketBuffer) -> Result<()> {
        loop {
            req_buffer.pos = 0;
            let (packet_size, _) = match self.up_socket.recv_from(&mut req_buffer.buf) {
                Ok((packet_size, source_address)) => (packet_size, source_address),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            };
            req_buffer.len = packet_size;

            match DnsPacket::from_buffer(req_buffer) {
                Ok(dns_packet) => {
                    if let Err(e) = self.handle_response(&dns_packet) {
                        log::error!("processing parent dns server error: {}", e);
                    }
                },
                Err(e) => 
                    log::error!("client_recv data format error: {}", e),
            };
        }

        Ok(())
    }

    fn handle_query(&mut self, query: &Query) -> Result<()> {
        log::debug!("Received query: {:?}", query.question);

        // 尝试本地查找
        if let Some(rec) = self.local_lookup(&query.question.name) {
            log::debug!("answer from local: {:?}", rec);
            let answers = [rec];
            self.response(ResultCode::NOERROR, &query, Some(&answers))?;
            return Ok(());
        }
        
        // 本地没找到, 而且也没有指定上级dns
        if self.up_dns_addr == IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)) {
            log::debug!("answer from local: {} not found, return refused", query.question.name);
            self.response(ResultCode::NXDOMAIN, &query, None)?;
            return Ok(());
        }

        // 转向上级dns服务器发起查询
        if self.queries.len() < MAX_QUERIES_LEN {
            let req_id = self.next_req_id();
            self.queries.insert(req_id, query.clone());
            self.send_request(&self.up_dns_addr, req_id, &query.question)
        } else {
            self.response(ResultCode::REFUSED, &query, None)
        }
    }

    /// 本地dns条目查询服务
    fn local_lookup(&self, qname: &str) -> Option<DnsRecord> {
        match self.hosts.get(qname) {
            Some(addr) => {
                Some(DnsRecord::A {
                    domain: String::from(qname),
                    addr: *addr,
                    ttl: self.ttl,
                })
            },
            None => None
        }
    }

    fn handle_response(&mut self, response: &DnsPacket) -> Result<()> {
        let query = match self.queries.remove(&response.header.id) {
            Some(c) => c,
            None => return Ok(()),
        };
        
        // 查询结果正确
        if !response.answers.is_empty() && response.header.rescode == ResultCode::NOERROR {
            // 非递归查询, 直接返回
            if query.forword == 0 {
                return self.response(response.header.rescode, &query, Some(&response.answers));
            }

            // 递归调用, 即插叙中, 遇到了ns服务名称需要解析的情况
            log::debug!("resolve: {:?} to {:?}", query.question, response.answers[0]);
            match self.queries.get(&query.forword) {
                Some(up_query) => {
                    match response.answers[0] {
                        DnsRecord::A {domain: _, ref addr, ttl: _} =>
                        return self.send_request(&IpAddr::V4(*addr), query.forword, &up_query.question),
                        _ => {
                            self.remove_recursive_query(query.forword);
                            return Err("dns server address is not ipv4".into());
                        },
                    }
                },
                None => return Err("parent query record not found".into()),
            };
        }

        // NXDOMAIN表示该域名不存在
        if response.header.rescode == ResultCode::NXDOMAIN {
            if query.forword == 0 {
                return self.response(response.header.rescode, &query, Some(&response.answers));
            }

            match self.remove_recursive_query(query.forword) {
                Some(ref top_query) => return self.response(response.header.rescode, top_query, Some(&response.answers)),
                None => return Err("top query record not found".into()),
            }
        }

        // 递归查询次数限制
        if query.count.get() > MAX_FORWARD_COUNT {
            return self.response(ResultCode::REFUSED, &query, None);
        }

        // 否则, 尝试用新的dns服务器再次进行查找
        if let Some(new_ns) = response.get_resolved_ns(&query.question.name) {
            query.count.set(query.count.get() + 1);
            self.queries.insert(response.header.id, query.clone());
            return self.send_request(&IpAddr::V4(new_ns), response.header.id, &query.question);
        }

        // 如果解析NS记录的ip失败。则尝试解析NS别名
        let new_ns_name = match response.get_unresolved_ns(&query.question.name) {
            Some(x) => x,
            None => return self.response(ResultCode::REFUSED, &query, None),
        };

        // 先要解析处ns服务器别名对应的ip, 才能继续解析之前的请求
        let new_query = Query::new(QueryData {
            id: 0,
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
            question: DnsQuestion { name: String::from(new_ns_name), qtype: QueryType::A },
            forword: response.header.id,
            expire: expire_of_unix(),
            count: Cell::new(query.count.get() + 1),
        });
        let new_req_id = self.next_req_id();
        self.queries.insert(response.header.id, query.clone());
        self.queries.insert(new_req_id, new_query.clone());
        self.send_request(&self.up_dns_addr, new_req_id, &new_query.question)

    }

    fn send_request(&self, dns_addr: &IpAddr, req_id: u16, question: &DnsQuestion) -> Result<()> {
        log::debug!("Attempting lookup of {:?} {} with ns {}",
                question.qtype, question.name, dns_addr);

        let mut packet = DnsPacket::new();
        packet.header.id = req_id;
        packet.header.questions = 1;
        packet.header.recursion_desired = true;
        packet.questions.push(question.clone());

        let mut req_buffer = BytePacketBuffer::new();
        packet.write(&mut req_buffer)?;
        self.up_socket.send_to(&req_buffer.buf[..req_buffer.pos], SocketAddr::new(*dns_addr, 53))?;

        Ok(())
    }

    /// 向查询客户端回复查询结果
    fn response(&self, resp_code: ResultCode, query: &Query, answers: Option<&[DnsRecord]>) -> Result<()> {
        let mut res_packet = DnsPacket::new();
        res_packet.header.id = query.id;
        res_packet.header.rescode = resp_code;
        res_packet.header.recursion_desired = true;
        res_packet.header.recursion_available = true;
        res_packet.header.response = true;
        res_packet.questions.push(query.question.clone());

        if let Some(answers) = answers {
            for rec in answers {
                log::debug!("Answer: {:?}", rec);
                res_packet.answers.push(rec.clone());
            }
        }
        
        let mut res_buffer = BytePacketBuffer::new();
        res_packet.write(&mut res_buffer)?;

        let len = res_buffer.pos();
        let data = res_buffer.get_range(0, len)?;
        
        self.socket.send_to(data, query.addr)?;

        Ok(())
    }

    /// 递归删除指定查询id的所有待查询项
    fn remove_recursive_query(&mut self, id: u16) -> Option<Query> {
        let mut tmp_id = id;
        loop {
            match self.queries.remove(&tmp_id) {
                Some(x) => {
                    if x.forword > 0 {
                        tmp_id = x.forword;
                    } else {
                        return Some(x);
                    }
                },
                None => return None,
            }
        }
    }

    /// 清理待查询队列, 将所有超时的查询项删除
    fn clear_queries_of_timeout(&mut self) {
        let now = now_of_unix();
        
        self.queries.retain(|k, v| {
            let keep = now <= v.expire;
            if !keep {
                log::trace!("request id {} is timeout, remove it", k);
            }
            keep
        });
    }

    /// 获取下一个查询请求id
    fn next_req_id(&mut self) -> u16 {
        self.curr_req_id = self.curr_req_id.wrapping_add(1);
        self.curr_req_id
    }

    /// 动态dns更新函数
    fn dyn_dns(&mut self, req_buffer: &BytePacketBuffer, rep_addr: &SocketAddr) -> Result<bool> {
        if req_buffer.len < C_DYNDNS_MIN_LEN
                || &req_buffer.buf[..C_DNYDNS_MAGIC.len()] != C_DNYDNS_MAGIC {
            return Ok(false);
        }

        // 解析包        
        let text = String::from_utf8_lossy(&req_buffer.buf[.. req_buffer.len]);
        log::debug!("dyndns packet received: {}", text);
        let params: Vec<&str> = text.split(' ').collect();

        // 校验参数数量
        if params.len() < C_DYNDNS_PARAM_COUNT {
            log::info!("dyndns packet format error");
            self.socket.send_to("error".as_bytes(), *rep_addr)?;
            return Ok(true);
        }
        
        log::debug!("dyndns packet: DIGEST = {}, ID = {}, HOST = {}, IP = {}",
                params[C_DYNDNS_PARAM_DIGEST],
                params[C_DYNDNS_PARAM_ID],
                params[C_DYNDNS_PARAM_HOST],
                params[C_DYNDNS_PARAM_IP]);
        
        // 校验参数md5
        if !check_dyndns_md5(&params, &self.key) {
            log::info!("dyndns packet checksum error");
            self.socket.send_to("error".as_bytes(), *rep_addr)?;
            return Ok(true);
        }
        // 校验参数提交时间
        if !check_dyndns_time(&params[C_DYNDNS_PARAM_ID])? {
            log::info!("dyndns packet time error");
            self.socket.send_to("error".as_bytes(), *rep_addr)?;
            return Ok(true);
        }

        let ip = match params[C_DYNDNS_PARAM_IP] {
            "0.0.0.0" => rep_addr.ip().to_string(),
            s => s.to_string(),
        };

        self.register_host(params[C_DYNDNS_PARAM_HOST], &ip)?;

        let rep = format!("{} {}", params[C_DYNDNS_PARAM_HOST], ip);
        self.socket.send_to(rep.as_bytes(), *rep_addr)?;

        Ok(true)
    }

}

/// 得到当前时间的unix时间表示(自1970-01-01以来的秒数)
fn now_of_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// 基于当前时间的过期时间
fn expire_of_unix() -> u64 {
    now_of_unix() + QUERY_TIMEOUT
}

fn check_dyndns_md5(params: &Vec<&str>, key: &str) -> bool { 
    let mut ctx = md5::Context::new();
    ctx.consume(params[C_DYNDNS_PARAM_ID].as_bytes());
    ctx.consume(params[C_DYNDNS_PARAM_HOST].as_bytes());
    ctx.consume(params[C_DYNDNS_PARAM_IP].as_bytes());
    ctx.consume(key.as_bytes());
    let hash = format!("{:x}", ctx.compute());

    let result = params[C_DYNDNS_PARAM_DIGEST] == hash;
    if !result {
        log::debug!("dyndns packet checksum error: expect {} but {}", params[C_DYNDNS_PARAM_DIGEST], hash);
    }
    result
}

fn check_dyndns_time(id: &str) -> Result<bool> {
    let now = now_of_unix() - C_2023_01_01;
    let id_num: u64 = id.parse()?;
    Ok(id_num <= now + C_DYNDNS_TIME_RANGE && id_num >= now - C_DYNDNS_TIME_RANGE)
}