package com.lurk.statistics.command;

import org.telegram.telegrambots.meta.api.methods.send.SendMessage;

public interface LurkCommand {

    /*
     * Executes command for a particular Telegram Chat Id.
     */
    public SendMessage execute(long chatId);

    /*
     * Returns relative HTTP path for this command.
     */
    public String path();

}
