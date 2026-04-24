--
-- PostgreSQL database dump
--

-- Dumped from database version 16.3 (Homebrew)
-- Dumped by pg_dump version 16.3 (Homebrew)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: accounts; Type: TABLE; Schema: public; Owner: peter
--

CREATE TABLE public.accounts (
    player_id integer NOT NULL,
    account_name character varying(50) NOT NULL,
    password character varying(1000) NOT NULL,
    email character varying(255) NOT NULL,
    created_at timestamp with time zone NOT NULL,
    last_login timestamp with time zone,
    selected_class boolean DEFAULT false NOT NULL
);


ALTER TABLE public.accounts OWNER TO peter;

--
-- Name: accounts_user_id_seq; Type: SEQUENCE; Schema: public; Owner: peter
--

CREATE SEQUENCE public.accounts_user_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.accounts_user_id_seq OWNER TO peter;

--
-- Name: accounts_user_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: peter
--

ALTER SEQUENCE public.accounts_user_id_seq OWNED BY public.accounts.player_id;


--
-- Name: accounts player_id; Type: DEFAULT; Schema: public; Owner: peter
--

ALTER TABLE ONLY public.accounts ALTER COLUMN player_id SET DEFAULT nextval('public.accounts_user_id_seq'::regclass);


--
-- Name: accounts accounts_email_key; Type: CONSTRAINT; Schema: public; Owner: peter
--

ALTER TABLE ONLY public.accounts
    ADD CONSTRAINT accounts_email_key UNIQUE (email);


--
-- Name: accounts accounts_pkey; Type: CONSTRAINT; Schema: public; Owner: peter
--

ALTER TABLE ONLY public.accounts
    ADD CONSTRAINT accounts_pkey PRIMARY KEY (player_id);


--
-- Name: accounts accounts_account_name_key; Type: CONSTRAINT; Schema: public; Owner: peter
--

ALTER TABLE ONLY public.accounts
    ADD CONSTRAINT accounts_account_name_key UNIQUE (account_name);


CREATE TABLE public.sessions (
    player_id integer NOT NULL,
    session character varying(255) NOT NULL,
    created_at timestamp with time zone NOT NULL,
    last_login timestamp with time zone
);

CREATE TABLE public.scores (
    id SERIAL PRIMARY KEY,            -- unique row id
    player_id INTEGER NOT NULL,          -- player identifier (UUID is common in games, could also be BIGINT)
    hero_name TEXT NOT NULL,          -- hero's name
    hero_rank TEXT NOT NULL,       -- rank or level
    total_xp BIGINT NOT NULL,         -- accumulated XP
    total_score INTEGER NOT NULL DEFAULT 0,
    score_survival INTEGER NOT NULL DEFAULT 0,
    score_progression INTEGER NOT NULL DEFAULT 0,
    score_wealth INTEGER NOT NULL DEFAULT 0,
    score_defense INTEGER NOT NULL DEFAULT 0,
    score_valor INTEGER NOT NULL DEFAULT 0,
    score_legacy INTEGER NOT NULL DEFAULT 0,
    days_survived INTEGER NOT NULL DEFAULT 0,
    highest_pressure_level INTEGER NOT NULL DEFAULT 0,
    waves_survived INTEGER NOT NULL DEFAULT 0,
    legendary_kills INTEGER NOT NULL DEFAULT 0,
    hideouts_cleared INTEGER NOT NULL DEFAULT 0,
    fate TEXT NOT NULL,               -- descriptive sentence of hero's fate
    crisis_tier INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW() -- when this score was recorded
);


--
-- PostgreSQL database dump complete
--

